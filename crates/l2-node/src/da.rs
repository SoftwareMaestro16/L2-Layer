use crate::config::NodeConfig;
use crate::storage::{DynStorage, StorageError, StoredBatchPayload};
use async_trait::async_trait;
use l2_core::{canonical_batch_data_bytes, canonical_batch_data_hash, Hash32, L2Block};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;

pub const DEFAULT_DA_MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_DA_PUBLIC_BACKEND: &str = "postgres";
pub const DEFAULT_DA_PUBLIC_FS_DIR: &str = "build/da-public";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataAvailabilityConfig {
    pub max_payload_bytes: usize,
    pub public_backend: PublicDaBackend,
}

impl DataAvailabilityConfig {
    pub fn from_node_config(config: &NodeConfig) -> Self {
        Self {
            max_payload_bytes: config.da_max_payload_bytes,
            public_backend: match config.da_public_backend.as_str() {
                "filesystem" => PublicDaBackend::Filesystem {
                    root_dir: config.da_public_fs_dir.clone(),
                    base_url: config.da_public_base_url.clone(),
                },
                _ => PublicDaBackend::PostgresOnly,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicDaBackend {
    PostgresOnly,
    Filesystem {
        root_dir: PathBuf,
        base_url: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchDaRef {
    pub block_height: u64,
    pub block_hash: Hash32,
    pub data_hash: Hash32,
    pub payload_size: usize,
    pub public_ref: Option<String>,
    pub public_uri: Option<String>,
}

#[async_trait]
pub trait DaWriter: Send + Sync {
    async fn write_batch_payload(&self, block: &L2Block) -> Result<BatchDaRef, DaError>;
}

#[async_trait]
pub trait DaReader: Send + Sync {
    async fn read_batch_payload(
        &self,
        block_height: u64,
    ) -> Result<Option<StoredBatchPayload>, DaError>;

    async fn read_batch_payload_by_hash(
        &self,
        block_height: u64,
        data_hash: Hash32,
    ) -> Result<Option<StoredBatchPayload>, DaError> {
        let Some(payload) = self.read_batch_payload(block_height).await? else {
            return Ok(None);
        };
        Ok((payload.data_hash == data_hash).then_some(payload))
    }
}

#[async_trait]
pub trait DaVerifier: Send + Sync {
    async fn verify_batch_payload(&self, block: &L2Block) -> Result<BatchDaRef, DaError>;
}

pub trait DataAvailability: DaWriter + DaReader + DaVerifier {}

impl<T> DataAvailability for T where T: DaWriter + DaReader + DaVerifier {}

pub type DynDa = Arc<dyn DataAvailability>;

#[derive(Clone)]
pub struct StorageDaStore {
    storage: DynStorage,
    config: DataAvailabilityConfig,
}

impl StorageDaStore {
    pub fn new(storage: DynStorage, config: DataAvailabilityConfig) -> Self {
        Self { storage, config }
    }

    fn payload_for_block(&self, block: &L2Block) -> Result<Vec<u8>, DaError> {
        let payload = canonical_batch_data_bytes(&block.transactions, &block.receipts);
        if payload.len() > self.config.max_payload_bytes {
            return Err(DaError::PayloadTooLarge {
                bytes: payload.len(),
                max: self.config.max_payload_bytes,
            });
        }
        let actual = canonical_batch_data_hash(&block.transactions, &block.receipts);
        if actual != block.header.data_hash {
            return Err(DaError::HashMismatch {
                expected: block.header.data_hash,
                actual,
            });
        }
        Ok(payload)
    }

    fn record_for_block(&self, block: &L2Block) -> Result<StoredBatchPayload, DaError> {
        Ok(StoredBatchPayload {
            block_height: block.header.height,
            block_hash: block.header.block_hash(),
            data_hash: block.header.data_hash,
            payload_bytes: self.payload_for_block(block)?,
            public_ref: None,
            public_uri: None,
        })
    }

    async fn publish_public_payload(&self, record: &mut StoredBatchPayload) -> Result<(), DaError> {
        let PublicDaBackend::Filesystem { root_dir, base_url } = &self.config.public_backend else {
            return Ok(());
        };
        let public_ref =
            public_payload_ref(record.block_height, record.block_hash, record.data_hash);
        let path = public_payload_path(root_dir, &public_ref);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        write_public_payload_file(&path, record).await?;
        record.public_uri = public_uri(base_url.as_deref(), &public_ref);
        record.public_ref = Some(public_ref);
        Ok(())
    }

    async fn verify_public_payload(&self, expected: &StoredBatchPayload) -> Result<(), DaError> {
        let PublicDaBackend::Filesystem { root_dir, .. } = &self.config.public_backend else {
            return Ok(());
        };
        let public_ref = expected.public_ref.clone().unwrap_or_else(|| {
            public_payload_ref(
                expected.block_height,
                expected.block_hash,
                expected.data_hash,
            )
        });
        let path = public_payload_path(root_dir, &public_ref);
        let payload = read_public_payload_file(&path, self.config.max_payload_bytes).await?;
        verify_payload_bytes(expected.data_hash, &payload)?;
        if payload != expected.payload_bytes {
            let actual = l2_core::crypto::hash_domain("l2.batch.data.v1", &[&payload]);
            return Err(DaError::HashMismatch {
                expected: expected.data_hash,
                actual,
            });
        }
        Ok(())
    }

    async fn read_public_payload_by_hash(
        &self,
        block_height: u64,
        data_hash: Hash32,
    ) -> Result<Option<StoredBatchPayload>, DaError> {
        let PublicDaBackend::Filesystem { root_dir, base_url } = &self.config.public_backend else {
            return Ok(None);
        };
        let height_dir = root_dir.join("blocks").join(block_height.to_string());
        let mut entries = match fs::read_dir(&height_dir).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let suffix = format!("-{}.el2batch", data_hash.to_hex());
        let mut matched = None;
        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if !file_name.ends_with(&suffix) {
                continue;
            }
            let block_hash_hex = &file_name[..file_name.len() - suffix.len()];
            let block_hash =
                Hash32::from_hex(block_hash_hex).map_err(|_| DaError::InvalidPublicReference)?;
            if matched.is_some() {
                return Err(DaError::AmbiguousPublicPayload);
            }
            let payload_bytes =
                read_public_payload_file(&entry.path(), self.config.max_payload_bytes).await?;
            verify_payload_bytes(data_hash, &payload_bytes)?;
            let public_ref = public_payload_ref(block_height, block_hash, data_hash);
            matched = Some(StoredBatchPayload {
                block_height,
                block_hash,
                data_hash,
                payload_bytes,
                public_uri: public_uri(base_url.as_deref(), &public_ref),
                public_ref: Some(public_ref),
            });
        }
        Ok(matched)
    }
}

#[async_trait]
impl DaWriter for StorageDaStore {
    async fn write_batch_payload(&self, block: &L2Block) -> Result<BatchDaRef, DaError> {
        let mut record = self.record_for_block(block)?;
        let payload_size = record.payload_bytes.len();
        self.publish_public_payload(&mut record).await?;
        self.storage.save_batch_payload(record.clone()).await?;
        Ok(BatchDaRef {
            block_height: record.block_height,
            block_hash: record.block_hash,
            data_hash: record.data_hash,
            payload_size,
            public_ref: record.public_ref,
            public_uri: record.public_uri,
        })
    }
}

#[async_trait]
impl DaReader for StorageDaStore {
    async fn read_batch_payload(
        &self,
        block_height: u64,
    ) -> Result<Option<StoredBatchPayload>, DaError> {
        Ok(self.storage.get_batch_payload(block_height).await?)
    }

    async fn read_batch_payload_by_hash(
        &self,
        block_height: u64,
        data_hash: Hash32,
    ) -> Result<Option<StoredBatchPayload>, DaError> {
        if let Some(stored) = self.read_batch_payload(block_height).await? {
            if stored.data_hash == data_hash {
                return Ok(Some(stored));
            }
        }
        self.read_public_payload_by_hash(block_height, data_hash)
            .await
    }
}

#[async_trait]
impl DaVerifier for StorageDaStore {
    async fn verify_batch_payload(&self, block: &L2Block) -> Result<BatchDaRef, DaError> {
        let expected = self.record_for_block(block)?;
        let Some(stored) = self.read_batch_payload(block.header.height).await? else {
            return Err(DaError::Unavailable);
        };
        if stored.block_hash != expected.block_hash {
            return Err(DaError::BlockHashMismatch {
                expected: expected.block_hash,
                actual: stored.block_hash,
            });
        }
        if stored.data_hash != expected.data_hash {
            return Err(DaError::HashMismatch {
                expected: expected.data_hash,
                actual: stored.data_hash,
            });
        }
        if stored.payload_bytes != expected.payload_bytes {
            let actual = l2_core::crypto::hash_domain("l2.batch.data.v1", &[&stored.payload_bytes]);
            return Err(DaError::HashMismatch {
                expected: expected.data_hash,
                actual,
            });
        }
        self.verify_public_payload(&expected).await?;
        Ok(BatchDaRef {
            block_height: expected.block_height,
            block_hash: expected.block_hash,
            data_hash: expected.data_hash,
            payload_size: expected.payload_bytes.len(),
            public_ref: stored.public_ref.or(expected.public_ref),
            public_uri: stored.public_uri.or(expected.public_uri),
        })
    }
}

fn public_payload_ref(block_height: u64, block_hash: Hash32, data_hash: Hash32) -> String {
    format!(
        "blocks/{}/{}-{}.el2batch",
        block_height,
        block_hash.to_hex(),
        data_hash.to_hex()
    )
}

fn public_payload_path(root_dir: &Path, public_ref: &str) -> PathBuf {
    let mut path = root_dir.to_path_buf();
    for segment in public_ref.split('/') {
        path.push(segment);
    }
    path
}

fn public_uri(base_url: Option<&str>, public_ref: &str) -> Option<String> {
    base_url.map(|base_url| {
        format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            public_ref.replace('\\', "/")
        )
    })
}

async fn write_public_payload_file(
    path: &Path,
    record: &StoredBatchPayload,
) -> Result<(), DaError> {
    match fs::read(path).await {
        Ok(existing) if existing == record.payload_bytes => return Ok(()),
        Ok(existing) => {
            verify_payload_bytes(record.data_hash, &existing)?;
            return Err(DaError::HashMismatch {
                expected: record.data_hash,
                actual: l2_core::crypto::hash_domain("l2.batch.data.v1", &[&existing]),
            });
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let tmp_path = path.with_extension(format!("{}.tmp", record.block_hash.to_hex()));
    fs::write(&tmp_path, &record.payload_bytes).await?;
    match fs::rename(&tmp_path, path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&tmp_path).await;
            let existing = fs::read(path).await?;
            if existing == record.payload_bytes {
                Ok(())
            } else {
                Err(DaError::HashMismatch {
                    expected: record.data_hash,
                    actual: l2_core::crypto::hash_domain("l2.batch.data.v1", &[&existing]),
                })
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&tmp_path).await;
            Err(error.into())
        }
    }
}

async fn read_public_payload_file(
    path: &Path,
    max_payload_bytes: usize,
) -> Result<Vec<u8>, DaError> {
    let payload = match fs::read(path).await {
        Ok(payload) => payload,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(DaError::Unavailable);
        }
        Err(error) => return Err(error.into()),
    };
    if payload.len() > max_payload_bytes {
        return Err(DaError::PayloadTooLarge {
            bytes: payload.len(),
            max: max_payload_bytes,
        });
    }
    Ok(payload)
}

fn verify_payload_bytes(expected: Hash32, payload: &[u8]) -> Result<(), DaError> {
    let actual = l2_core::crypto::hash_domain("l2.batch.data.v1", &[payload]);
    if actual != expected {
        return Err(DaError::HashMismatch { expected, actual });
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum DaError {
    #[error("batch payload is unavailable")]
    Unavailable,
    #[error("batch payload is {bytes} bytes, max is {max} bytes")]
    PayloadTooLarge { bytes: usize, max: usize },
    #[error("batch payload hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: Hash32, actual: Hash32 },
    #[error("batch block hash mismatch: expected {expected}, got {actual}")]
    BlockHashMismatch { expected: Hash32, actual: Hash32 },
    #[error("batch public payload reference is invalid")]
    InvalidPublicReference,
    #[error("batch public payload reference is ambiguous")]
    AmbiguousPublicPayload,
    #[error("public DA filesystem failed: {0}")]
    PublicIo(#[from] std::io::Error),
    #[error("storage failed: {0}")]
    Storage(#[from] StorageError),
}

#[cfg(test)]
#[path = "da_tests.rs"]
mod tests;
