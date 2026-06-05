use super::{ApiError, AppState};
use crate::faucet::MAX_ENT_FAUCET_BATCH_CLAIMS;
use crate::storage::{EntFaucetClaimRecord, EntFaucetClaimStatus};
use axum::extract::{Query, State};
use axum::Json;
use l2_core::{l2_raw_address, l2_user_friendly_address, Hash32};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct FaucetExplorerQuery {
    pub(super) limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerFaucetBatches {
    pub(super) items: Vec<ExplorerFaucetBatch>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerFaucetBatch {
    pub(super) batch_id: Hash32,
    pub(super) claims_total: usize,
    pub(super) granted: usize,
    pub(super) duplicate_account: usize,
    pub(super) failed: usize,
    pub(super) claims: Vec<ExplorerFaucetClaim>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerFaucetClaim {
    pub(super) claim_index: u32,
    pub(super) claim_id: String,
    pub(super) account_id: Hash32,
    pub(super) account_raw_address: String,
    pub(super) account_friendly_address: String,
    #[serde(with = "l2_core::serde_u128_string")]
    pub(super) amount_base_units: u128,
    pub(super) deposit_id: Hash32,
    pub(super) status: &'static str,
}

pub(super) async fn explorer_faucet_batches(
    State(state): State<AppState>,
    Query(query): Query<FaucetExplorerQuery>,
) -> Result<Json<ExplorerFaucetBatches>, ApiError> {
    let limit = query
        .limit
        .unwrap_or(50)
        .clamp(1, MAX_ENT_FAUCET_BATCH_CLAIMS);
    let records = state.storage.list_ent_faucet_claims(limit as u32).await?;
    Ok(Json(ExplorerFaucetBatches {
        items: group_faucet_batches(records),
    }))
}

fn group_faucet_batches(records: Vec<EntFaucetClaimRecord>) -> Vec<ExplorerFaucetBatch> {
    let mut indexes = BTreeMap::<Hash32, usize>::new();
    let mut batches = Vec::<ExplorerFaucetBatch>::new();
    for record in records {
        let index = *indexes.entry(record.batch_id).or_insert_with(|| {
            batches.push(ExplorerFaucetBatch {
                batch_id: record.batch_id,
                claims_total: 0,
                granted: 0,
                duplicate_account: 0,
                failed: 0,
                claims: Vec::new(),
            });
            batches.len() - 1
        });
        let batch = &mut batches[index];
        batch.claims_total += 1;
        match record.status {
            EntFaucetClaimStatus::Granted => batch.granted += 1,
            EntFaucetClaimStatus::DuplicateAccount => batch.duplicate_account += 1,
            EntFaucetClaimStatus::Failed => batch.failed += 1,
        }
        batch.claims.push(explorer_faucet_claim(record));
    }
    batches
}

fn explorer_faucet_claim(record: EntFaucetClaimRecord) -> ExplorerFaucetClaim {
    ExplorerFaucetClaim {
        claim_index: record.claim_index,
        claim_id: record.claim_id,
        account_id: record.account_id,
        account_raw_address: l2_raw_address(record.account_id),
        account_friendly_address: l2_user_friendly_address(record.account_id),
        amount_base_units: record.amount_base_units,
        deposit_id: record.deposit_id,
        status: record.status.as_str(),
    }
}
