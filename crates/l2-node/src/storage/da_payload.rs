use super::StoredBatchPayload;

impl StoredBatchPayload {
    pub(crate) fn has_same_canonical_payload(&self, other: &Self) -> bool {
        self.block_height == other.block_height
            && self.block_hash == other.block_hash
            && self.data_hash == other.data_hash
            && self.payload_bytes == other.payload_bytes
    }

    pub(crate) fn has_public_ref_conflict(&self, other: &Self) -> bool {
        matches!(
            (&self.public_ref, &other.public_ref),
            (Some(existing), Some(incoming)) if existing != incoming
        )
    }

    pub(crate) fn merge_public_metadata_from(&mut self, incoming: &Self) -> bool {
        let mut updated = false;
        if self.public_ref.is_none() && incoming.public_ref.is_some() {
            self.public_ref = incoming.public_ref.clone();
            updated = true;
        }
        if incoming.public_uri.is_some() && self.public_uri != incoming.public_uri {
            self.public_uri = incoming.public_uri.clone();
            updated = true;
        }
        updated
    }
}
