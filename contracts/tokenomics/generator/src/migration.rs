use crate::state::{Config, CONFIG};
use astroport::asset::{addr_validate_to_lower, AssetInfo};

use cosmwasm_std::{Addr, Decimal, DepsMut, StdError, Uint128, Uint64};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// This structure stores the parameters for a generator (in the upgraded version of the Generator contract).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfoV100 {
    /// This is the share of ASTRO rewards that this generator receives every block
    pub alloc_point: Uint64,
    /// Accumulated amount of rewards per LP unit. Used for reward calculations
    pub last_reward_block: Uint64,
    /// This is the accrued amount of rewards up to the latest checkpoint
    pub accumulated_rewards_per_share: Decimal,
    /// The 3rd party proxy reward contract
    pub reward_proxy: Option<Addr>,
    /// This is the accrued amount of 3rd party rewards up to the latest checkpoint
    pub accumulated_proxy_rewards_per_share: Decimal,
    /// This is the balance of 3rd party proxy rewards that the proxy had before a reward snapshot
    pub proxy_reward_balance_before_update: Uint128,
    /// The orphaned proxy rewards which are left behind by emergency withdrawals
    pub orphan_proxy_rewards: Uint128,
}

/// Stores the contract config(V1.0.0) at the given key
pub const POOL_INFOV100: Map<&Addr, PoolInfoV100> = Map::new("pool_info");

/// This structure stores the parameters for a generator (in the upgraded version v1.1.0 of the Generator contract).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfoV110 {
    /// Allocation point is used to control reward distribution among the pools
    pub alloc_point: Uint64,
    /// Accumulated amount of reward per share unit. Used for reward calculations
    pub last_reward_block: Uint64,
    pub accumulated_rewards_per_share: Decimal,
    /// the reward proxy contract
    pub reward_proxy: Option<Addr>,
    pub accumulated_proxy_rewards_per_share: Decimal,
    /// for calculation of new proxy rewards
    pub proxy_reward_balance_before_update: Uint128,
    /// the orphan proxy rewards which are left by emergency withdrawals
    pub orphan_proxy_rewards: Uint128,
    /// The pool has assets giving additional rewards
    pub has_asset_rewards: bool,
}

/// Stores the contract config(V1.1.0) at the given key
pub const POOL_INFOV110: Map<&Addr, PoolInfoV110> = Map::new("pool_info");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfoV120 {
    /// Accumulated amount of reward per share unit. Used for reward calculations
    pub last_reward_block: Uint64,
    pub accumulated_rewards_per_share: Decimal,
    /// the reward proxy contract
    pub reward_proxy: Option<Addr>,
    pub accumulated_proxy_rewards_per_share: Decimal,
    /// for calculation of new proxy rewards
    pub proxy_reward_balance_before_update: Uint128,
    /// the orphan proxy rewards which are left by emergency withdrawals
    pub orphan_proxy_rewards: Uint128,
    /// The pool has assets giving additional rewards
    pub has_asset_rewards: bool,
}

/// Stores the contract config(V1.2.0) at the given key
pub const POOL_INFOV120: Map<&Addr, PoolInfoV120> = Map::new("pool_info");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfoV130 {
    /// Accumulated amount of reward per share unit. Used for reward calculations
    pub last_reward_block: Uint64,
    pub accumulated_rewards_per_share: Decimal,
    /// the reward proxy contract
    pub reward_proxy: Option<Addr>,
    pub accumulated_proxy_rewards_per_share: Decimal,
    /// for calculation of new proxy rewards
    pub proxy_reward_balance_before_update: Uint128,
    /// the orphan proxy rewards which are left by emergency withdrawals
    pub orphan_proxy_rewards: Uint128,
    /// The pool has assets giving additional rewards
    pub has_asset_rewards: bool,
}

/// Stores the contract config(V1.3.0) at the given key
pub const POOL_INFOV130: Map<&Addr, PoolInfoV130> = Map::new("pool_info");

/// This structure describes the main control config of generator.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigV100 {
    /// Contract address that used for controls settings
    pub owner: Addr,
    /// The ASTRO token address
    pub astro_token: Addr,
    /// Total amount of ASTRO rewards per block
    pub tokens_per_block: Uint128,
    /// The total allocation points. Must be the sum of all allocation points in all pools.
    pub total_alloc_point: Uint64,
    /// The block number when ASTRO mining starts.
    pub start_block: Uint64,
    /// The list of allowed reward proxy contracts
    pub allowed_reward_proxies: Vec<Addr>,
    /// The vesting contract from which rewards are distributed
    pub vesting_contract: Addr,
}

/// Stores the contract config(V1.1.0) at the given key
pub const CONFIGV100: Item<ConfigV100> = Item::new("config");

/// This structure describes a contract migration message.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrationMsgV120 {
    /// The Factory address
    pub factory: String,
    /// Contract address which can only set active generators and their alloc points
    pub generator_controller: Option<String>,
    /// The blocked list of tokens
    pub blocked_list_tokens: Option<Vec<AssetInfo>>,
    /// The guardian address
    pub guardian: Option<String>,
}

/// Migrate config to V1.2.0
pub fn migrate_configs_to_v120(
    deps: &mut DepsMut,
    pools: Vec<(Addr, Uint64)>,
    msg: MigrationMsgV120,
) -> Result<(), StdError> {
    let cfg_100 = CONFIGV100.load(deps.storage)?;
    let pools = pools
        .into_iter()
        .map(|(addr, apoints)| (addr, apoints.into()))
        .collect();

    let mut cfg = Config {
        owner: cfg_100.owner,
        factory: addr_validate_to_lower(deps.api, &msg.factory)?,
        generator_controller: None,
        voting_escrow: None,
        astro_token: cfg_100.astro_token,
        tokens_per_block: cfg_100.tokens_per_block,
        total_alloc_point: cfg_100.total_alloc_point.into(),
        start_block: cfg_100.start_block,
        allowed_reward_proxies: cfg_100.allowed_reward_proxies,
        vesting_contract: cfg_100.vesting_contract,
        active_pools: pools,
        blocked_list_tokens: vec![],
        guardian: None,
        generator_limit: None,
    };

    if let Some(generator_controller) = msg.generator_controller {
        cfg.generator_controller = Some(addr_validate_to_lower(deps.api, &generator_controller)?);
    }

    if let Some(blocked_list_tokens) = msg.blocked_list_tokens {
        cfg.blocked_list_tokens = blocked_list_tokens;
    }

    if let Some(guardian) = msg.guardian {
        cfg.guardian = Some(addr_validate_to_lower(deps.api, &guardian)?);
    }

    CONFIG.save(deps.storage, &cfg)?;

    Ok(())
}

/// This structure describes the main control config of generator.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigV130 {
    /// Contract address that used for controls settings
    pub owner: Addr,
    /// The ASTRO token address
    pub astro_token: Addr,
    /// Total amount of ASTRO rewards per block
    pub tokens_per_block: Uint128,
    /// The total allocation points. Must be the sum of all allocation points in all pools.
    pub total_alloc_point: Uint64,
    /// The block number when ASTRO mining starts.
    pub start_block: Uint64,
    /// The list of allowed reward proxy contracts
    pub allowed_reward_proxies: Vec<Addr>,
    /// The vesting contract from which rewards are distributed
    pub vesting_contract: Addr,
    /// The list of active pools with allocation points
    pub active_pools: Vec<(Addr, Uint128)>,
}

/// Stores the contract config(V1.3.0) at the given key
pub const CONFIGV130: Item<ConfigV130> = Item::new("config");

/// This structure describes a contract migration message.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrationMsgV140 {
    /// The Factory address
    pub factory: String,
    /// Contract address which can only set active generators and their alloc points
    pub generator_controller: Option<String>,
    /// The blocked list of tokens
    pub blocked_list_tokens: Option<Vec<AssetInfo>>,
    /// The guardian address
    pub guardian: Option<String>,
    /// The voting escrow contract
    pub voting_escrow: Option<String>,
    /// The limit of generators
    pub generator_limit: Option<u32>,
}

/// Migrate config to V1.4.0
pub fn migrate_configs_to_v140(deps: &mut DepsMut, msg: MigrationMsgV140) -> Result<(), StdError> {
    let cfg_130 = CONFIGV130.load(deps.storage)?;

    let mut cfg = Config {
        owner: cfg_130.owner,
        factory: addr_validate_to_lower(deps.api, &msg.factory)?,
        generator_controller: None,
        voting_escrow: None,
        astro_token: cfg_130.astro_token,
        tokens_per_block: cfg_130.tokens_per_block,
        total_alloc_point: cfg_130.total_alloc_point.into(),
        start_block: cfg_130.start_block,
        allowed_reward_proxies: cfg_130.allowed_reward_proxies,
        vesting_contract: cfg_130.vesting_contract,
        active_pools: cfg_130.active_pools,
        blocked_list_tokens: vec![],
        guardian: None,
        generator_limit: None,
    };

    if let Some(generator_controller) = msg.generator_controller {
        cfg.generator_controller = Some(addr_validate_to_lower(deps.api, &generator_controller)?);
    }

    if let Some(blocked_list_tokens) = msg.blocked_list_tokens {
        cfg.blocked_list_tokens = blocked_list_tokens;
    }

    if let Some(guardian) = msg.guardian {
        cfg.guardian = Some(addr_validate_to_lower(deps.api, &guardian)?);
    }

    if let Some(voting_escrow) = msg.voting_escrow {
        cfg.voting_escrow = Some(addr_validate_to_lower(deps.api, &voting_escrow)?);
    }

    if let Some(generator_limit) = msg.generator_limit {
        cfg.generator_limit = Some(generator_limit);
    }

    CONFIG.save(deps.storage, &cfg)?;

    Ok(())
}
