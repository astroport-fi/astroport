use astroport::asset::AssetInfo;
use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

/// ## Description
/// This enum describes maker contract errors!
#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid bridge {0} to {1}")]
    InvalidBridge(AssetInfo, AssetInfo),

    #[error("Invalid bridge. Pool {0} to {1} not found")]
    InvalidBridgeNoPool(AssetInfo, AssetInfo),

    #[error("Invalid bridge destination. {0} cannot be swapped to ASTRO")]
    InvalidBridgeDestination(AssetInfo),

    #[error("Max bridge depth {0} is reached")]
    MaxBridgeDepth(u64),

    #[error("Cannot swap {0}. No swap destinations")]
    CannotSwap(AssetInfo),

    #[error("Incorrect governance percent of its share")]
    IncorrectGovernancePercent {},

    #[error("Incorrect max spread")]
    IncorrectMaxSpread {},

    #[error("Cannot collect. Remove duplicate asset")]
    DuplicatedAsset {},

    #[error("Rewards collecting is already enabled")]
    RewardsAlreadyEnabled {},

    #[error("An error occurred during migration")]
    MigrationError {},
}

impl From<OverflowError> for ContractError {
    fn from(o: OverflowError) -> Self {
        StdError::from(o).into()
    }
}
