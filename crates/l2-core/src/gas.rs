use crate::types::L2TransactionKind;
use serde::{Deserialize, Serialize};

pub const GAS_SCHEDULE_VERSION_V1: u32 = 1;
pub const DEFAULT_TRANSFER_GAS: u64 = 10;
pub const DEFAULT_WITHDRAW_GAS: u64 = 20;
pub const DEFAULT_CALL_CONTRACT_GAS: u64 = 50;
pub const DEFAULT_REJECTED_EXECUTION_GAS: u64 = 1;
pub const DEFAULT_MIN_GAS_PRICE: u128 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GasSchedule {
    pub version: u32,
    pub transfer_gas: u64,
    pub withdraw_gas: u64,
    pub call_contract_gas: u64,
    pub rejected_execution_gas: u64,
    pub min_gas_price: u128,
}

impl Default for GasSchedule {
    fn default() -> Self {
        Self {
            version: GAS_SCHEDULE_VERSION_V1,
            transfer_gas: DEFAULT_TRANSFER_GAS,
            withdraw_gas: DEFAULT_WITHDRAW_GAS,
            call_contract_gas: DEFAULT_CALL_CONTRACT_GAS,
            rejected_execution_gas: DEFAULT_REJECTED_EXECUTION_GAS,
            min_gas_price: DEFAULT_MIN_GAS_PRICE,
        }
    }
}

impl GasSchedule {
    pub fn validate(&self) -> Result<(), GasError> {
        if self.version != GAS_SCHEDULE_VERSION_V1 {
            return Err(GasError::UnsupportedVersion {
                version: self.version,
            });
        }
        if self.transfer_gas == 0
            || self.withdraw_gas == 0
            || self.call_contract_gas == 0
            || self.rejected_execution_gas == 0
        {
            return Err(GasError::ZeroGasCost);
        }
        if self.min_gas_price == 0 {
            return Err(GasError::ZeroMinGasPrice);
        }
        Ok(())
    }

    pub fn required_gas(&self, kind: &L2TransactionKind) -> u64 {
        match kind {
            L2TransactionKind::Deposit { .. } => 0,
            L2TransactionKind::Transfer { .. } => self.transfer_gas,
            L2TransactionKind::RotatePublicKey { .. } => self.transfer_gas,
            L2TransactionKind::Withdraw { .. } => self.withdraw_gas,
            L2TransactionKind::DeployContract { .. } => self.call_contract_gas,
            L2TransactionKind::CallContract { .. } => self.call_contract_gas,
            L2TransactionKind::InternalMessage { .. } => self.call_contract_gas,
        }
    }

    pub fn execution_fee(
        &self,
        kind: &L2TransactionKind,
        gas_limit: u64,
        max_gas_price: u128,
    ) -> Result<GasFee, GasError> {
        self.validate_min_gas_price(max_gas_price)?;
        let gas_units = self.required_gas(kind);
        if gas_limit < gas_units {
            return Err(GasError::InsufficientGasLimit {
                gas_limit,
                required: gas_units,
            });
        }
        self.fee_for_gas(gas_units, max_gas_price)
    }

    pub fn rejection_fee(&self, gas_limit: u64, max_gas_price: u128) -> Result<GasFee, GasError> {
        let gas_units = self.rejected_execution_gas.min(gas_limit);
        self.fee_for_gas(gas_units, max_gas_price)
    }

    pub fn fee_for_units(&self, gas_units: u64, max_gas_price: u128) -> Result<GasFee, GasError> {
        self.validate_min_gas_price(max_gas_price)?;
        self.fee_for_gas(gas_units, max_gas_price)
    }

    pub fn validate_min_gas_price(&self, max_gas_price: u128) -> Result<(), GasError> {
        if max_gas_price < self.min_gas_price {
            return Err(GasError::GasPriceTooLow {
                gas_price: max_gas_price,
                min: self.min_gas_price,
            });
        }
        Ok(())
    }

    fn fee_for_gas(&self, gas_units: u64, max_gas_price: u128) -> Result<GasFee, GasError> {
        let amount = u128::from(gas_units)
            .checked_mul(max_gas_price)
            .ok_or(GasError::FeeOverflow)?;
        Ok(GasFee { gas_units, amount })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GasFee {
    pub gas_units: u64,
    pub amount: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GasError {
    #[error("unsupported gas schedule version {version}")]
    UnsupportedVersion { version: u32 },
    #[error("gas costs must be non-zero")]
    ZeroGasCost,
    #[error("minimum gas price must be non-zero")]
    ZeroMinGasPrice,
    #[error("max_gas_price {gas_price} is below minimum {min}")]
    GasPriceTooLow { gas_price: u128, min: u128 },
    #[error("gas_limit {gas_limit} is below required gas {required}")]
    InsufficientGasLimit { gas_limit: u64, required: u64 },
    #[error("gas fee overflow")]
    FeeOverflow,
}

impl GasError {
    pub fn rejection_reason(self) -> &'static str {
        match self {
            Self::UnsupportedVersion { .. } => "unsupported_gas_schedule",
            Self::ZeroGasCost => "invalid_gas_schedule",
            Self::ZeroMinGasPrice => "invalid_gas_schedule",
            Self::GasPriceTooLow { .. } => "gas_price_too_low",
            Self::InsufficientGasLimit { .. } => "insufficient_gas_limit",
            Self::FeeOverflow => "fee_overflow",
        }
    }
}
