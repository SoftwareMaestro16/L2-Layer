use crate::consensus::account_leaf_hash;
use crate::crypto::Hash32;
use crate::merkle::merkle_root;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub nonce: u64,
    pub balances: BTreeMap<u32, u128>,
    pub code_hash: Hash32,
    pub data_hash: Hash32,
    pub storage_root: Hash32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_boc_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_boc_base64: Option<String>,
    pub last_lt: u64,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            nonce: 0,
            balances: BTreeMap::new(),
            code_hash: Hash32::ZERO,
            data_hash: Hash32::ZERO,
            storage_root: Hash32::ZERO,
            code_boc_base64: None,
            data_boc_base64: None,
            last_lt: 0,
        }
    }
}

impl Account {
    pub fn balance(&self, asset_id: u32) -> u128 {
        *self.balances.get(&asset_id).unwrap_or(&0)
    }

    pub fn can_credit(&self, asset_id: u32, amount: u128) -> bool {
        self.balance(asset_id).checked_add(amount).is_some()
    }

    pub fn credit(&mut self, asset_id: u32, amount: u128) -> bool {
        let balance = self.balances.entry(asset_id).or_default();
        let Some(next) = balance.checked_add(amount) else {
            return false;
        };
        *balance = next;
        true
    }

    pub fn debit(&mut self, asset_id: u32, amount: u128) -> bool {
        let balance = self.balances.entry(asset_id).or_default();
        if *balance < amount {
            return false;
        }
        *balance -= amount;
        true
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub accounts: BTreeMap<Hash32, Account>,
}

impl State {
    pub fn account(&self, id: Hash32) -> Option<&Account> {
        self.accounts.get(&id)
    }

    pub fn account_mut(&mut self, id: Hash32) -> &mut Account {
        self.accounts.entry(id).or_default()
    }

    pub fn root_hash(&self) -> Hash32 {
        let leaves = self
            .accounts
            .iter()
            .map(|(id, account)| account_leaf_hash(*id, account))
            .collect::<Vec<_>>();
        merkle_root(&leaves)
    }
}
