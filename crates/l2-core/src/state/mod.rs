use crate::consensus::account_leaf_hash;
use crate::crypto::Hash32;
use crate::merkle::merkle_root;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    #[default]
    User,
    Contract,
    System,
    Operator,
}

impl AccountType {
    pub fn consensus_tag(self) -> u8 {
        match self {
            Self::User => 1,
            Self::Contract => 2,
            Self::System => 3,
            Self::Operator => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountFlags {
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub contract_only: bool,
    #[serde(default)]
    pub system_only: bool,
}

impl AccountFlags {
    const DISABLED: u8 = 1 << 0;
    const CONTRACT_ONLY: u8 = 1 << 1;
    const SYSTEM_ONLY: u8 = 1 << 2;

    pub fn consensus_bits(self) -> u8 {
        let mut bits = 0u8;
        if self.disabled {
            bits |= Self::DISABLED;
        }
        if self.contract_only {
            bits |= Self::CONTRACT_ONLY;
        }
        if self.system_only {
            bits |= Self::SYSTEM_ONLY;
        }
        bits
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRecoveryLock {
    #[serde(default)]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin: Option<Hash32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Account {
    #[serde(default)]
    pub account_type: AccountType,
    #[serde(default)]
    pub flags: AccountFlags,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_public_key: Option<Hash32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_lock: Option<AccountRecoveryLock>,
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
            account_type: AccountType::User,
            flags: AccountFlags::default(),
            active_public_key: None,
            recovery_lock: None,
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

    pub fn can_initialize_contract(&self) -> bool {
        self.nonce == 0
            && matches!(self.account_type, AccountType::User | AccountType::Contract)
            && !self.flags.disabled
            && !self.flags.system_only
            && self.active_public_key.is_none()
            && !self.is_recovery_locked()
            && self.code_hash == Hash32::ZERO
            && self.data_hash == Hash32::ZERO
            && self.storage_root == Hash32::ZERO
            && self.code_boc_base64.is_none()
            && self.data_boc_base64.is_none()
    }

    pub fn can_send_public_transaction(&self) -> bool {
        matches!(self.account_type, AccountType::User | AccountType::Operator)
            && !self.flags.disabled
            && !self.flags.contract_only
            && !self.flags.system_only
            && !self.is_recovery_locked()
    }

    pub fn is_recovery_locked(&self) -> bool {
        self.recovery_lock.as_ref().is_some_and(|lock| lock.locked)
    }

    pub fn mark_contract_account(&mut self) {
        self.account_type = AccountType::Contract;
        self.flags.contract_only = true;
        self.active_public_key = None;
        self.recovery_lock = None;
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
