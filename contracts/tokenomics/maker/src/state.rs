use astroport::common::OwnershipProposal;
use cosmwasm_std::{Addr, Decimal, Uint64};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// ## Description
/// This structure describes the main control config of maker.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Sets contract address that used for controls settings
    pub owner: Addr,
    /// Sets the factory contract address
    pub factory_contract: Addr,
    /// Sets the staking contract address
    pub staking_contract: Addr,
    /// Sets the governance contract address
    pub governance_contract: Option<Addr>,
    /// Sets the governance percent
    pub governance_percent: Uint64,
    /// Sets the ASTRO token address
    pub astro_token_contract: Addr,
    /// Sets the max spread
    pub max_spread: Decimal,
}

/// ## Description
/// Stores config at the given key
pub const CONFIG: Item<Config> = Item::new("config");
/// ## Description
/// Contains proposal for change ownership.
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");
