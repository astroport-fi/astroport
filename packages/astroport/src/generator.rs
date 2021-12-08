use cosmwasm_std::{Addr, Uint128, Uint64};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub astro_token: String,
    pub tokens_per_block: Uint128,
    pub start_block: Uint64,
    pub allowed_reward_proxies: Vec<String>,
    pub vesting_contract: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig {
        vesting_contract: Option<String>,
    },
    Add {
        lp_token: Addr,
        alloc_point: Uint64,
        reward_proxy: Option<String>,
    },
    Set {
        lp_token: Addr,
        alloc_point: Uint64,
    },
    MassUpdatePools {},
    UpdatePool {
        lp_token: Addr,
    },
    Withdraw {
        lp_token: Addr,
        amount: Uint128,
    },
    EmergencyWithdraw {
        lp_token: Addr,
    },
    SetAllowedRewardProxies {
        proxies: Vec<String>,
    },
    SendOrphanProxyReward {
        recipient: String,
        lp_token: String,
    },
    Receive(Cw20ReceiveMsg),
    SetTokensPerBlock {
        amount: Uint128,
    },
    ProposeNewOwner {
        owner: String,
        expires_in: u64,
    },
    DropOwnershipProposal {},
    ClaimOwnership {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    PoolLength {},
    Deposit { lp_token: Addr, user: Addr },
    PendingToken { lp_token: Addr, user: Addr },
    Config {},
    RewardInfo { lp_token: Addr },
    OrphanProxyRewards { lp_token: Addr },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolLengthResponse {
    pub length: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PendingTokenResponse {
    pub pending: Uint128,
    pub pending_on_proxy: Option<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponse {
    pub base_reward_token: Addr,
    pub proxy_reward_token: Option<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: Addr,
    pub astro_token: Addr,
    pub tokens_per_block: Uint128,
    pub total_alloc_point: Uint64,
    pub start_block: Uint64,
    pub allowed_reward_proxies: Vec<Addr>,
    pub vesting_contract: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    Deposit {},
    DepositFor(Addr),
}
