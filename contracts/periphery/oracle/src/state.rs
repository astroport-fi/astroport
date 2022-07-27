use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::asset::{AssetInfo, PairInfo};
use cosmwasm_std::{Addr, Decimal256, Uint128};
use cw_storage_plus::Item;

/// ## Description
/// Stores the contract config at the given key
pub const CONFIG: Item<Config> = Item::new("config");
/// Stores the latest cumulative and average prices at the given key
pub const PRICE_LAST: Item<PriceCumulativeLast> = Item::new("price_last");

/// ## Description
/// This structure stores the latest cumulative and average token prices for the target pool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceCumulativeLast {
    /// The vector contains last cumulative prices for each pair of assets in the pool
    pub cumulative_prices: Vec<(AssetInfo, AssetInfo, Uint128)>,
    /// The vector contains average prices for each pair of assets in the pool
    pub average_prices: Vec<(AssetInfo, AssetInfo, Decimal256)>,
    /// The last timestamp block in pool
    pub block_timestamp_last: u64,
}

/// ## Description
/// Global configuration for the contract
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The address that's allowed to change contract parameters
    pub owner: Addr,
    /// The factory contract address
    pub factory: Addr,
    /// The assets in the pool. Each asset is described using a [`AssetInfo`]
    pub asset_infos: Vec<AssetInfo>,
    /// Information about the pair (LP token address, pair type etc)
    pub pair: PairInfo,
}
