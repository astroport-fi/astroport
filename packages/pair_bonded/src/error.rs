use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

/// This enum describes stableswap pair contract errors
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Doubling assets in asset infos")]
    DoublingAssets {},

    #[error("Provided spread amount exceeds allowed limit")]
    AllowedSpreadAssertion {},

    #[error("Operation exceeds max spread limit")]
    MaxSpreadAssertion {},

    #[error("Native token balance mismatch between the argument and the transferred")]
    AssetMismatch {},

    #[error("You need to provide init params")]
    InitParamsNotFound {},

    #[error("Pair is not migrated to the new admin!")]
    PairIsNotMigrated {},

    #[error("Operation is not supported for this pool.")]
    NotSupported {},
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
