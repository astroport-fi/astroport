use astroport::asset::MINIMUM_LIQUIDITY_AMOUNT;
use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

/// ## Description
/// This enum describes pair contract errors!
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Operation non supported")]
    NonSupported {},

    #[error("Event of zero transfer")]
    InvalidZeroAmount {},

    #[error("Operation exceeds max spread limit")]
    MaxSpreadAssertion {},

    #[error("Provided spread amount exceeds allowed limit")]
    AllowedSpreadAssertion {},

    #[error("Operation exceeds max splippage tolerance")]
    MaxSlippageAssertion {},

    #[error("Doubling assets in asset infos")]
    DoublingAssets {},

    #[error("Asset mismatch between the requested and the stored asset in contract")]
    AssetMismatch {},

    #[error("Generator address is not set in factory. Cannot auto-stake")]
    AutoStakeError {},

    #[error("Pair is not migrated to the new admin!")]
    PairIsNotMigrated {},

    #[error(
        "The minimum amount for a provide liquidity operation must be greater or equal to {0}",
        MINIMUM_LIQUIDITY_AMOUNT
    )]
    MinimumLiquidityAmountError {},
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
