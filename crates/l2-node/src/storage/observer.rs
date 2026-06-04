use l2_core::{Hash32, State};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObserverCheckpoint {
    pub next_batch_no: u64,
    pub next_block_height: u64,
    pub state_root: Hash32,
    pub state: State,
}

impl ObserverCheckpoint {
    pub fn genesis() -> Self {
        let state = State::default();
        Self {
            next_batch_no: 1,
            next_block_height: 0,
            state_root: state.root_hash(),
            state,
        }
    }

    pub fn validate_integrity(&self) -> bool {
        self.state.root_hash() == self.state_root
    }
}
