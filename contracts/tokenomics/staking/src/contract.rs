use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128, WasmMsg,
};

use crate::error::ContractError;
use crate::state::{Config, CONFIG};
use astroport::staking::{ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use cw2::set_contract_version;
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse, TokenInfoResponse};

use astroport::hook::InitHook;
use astroport::token::InstantiateMsg as TokenInstantiateMsg;

// version info for migration info
const CONTRACT_NAME: &str = "astroport-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const TOKEN_NAME: &str = "astroport-staking-token";
const TOKEN_SYMBOL: &str = "xASTRO";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Store config
    CONFIG.save(
        deps.storage,
        &Config {
            deposit_token_addr: deps.api.addr_validate(&msg.deposit_token_addr)?,
            share_token_addr: Addr::unchecked(""),
        },
    )?;

    // Create token
    let resp = Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Instantiate {
        admin: None,
        code_id: msg.token_code_id,
        msg: to_binary(&TokenInstantiateMsg {
            name: TOKEN_NAME.to_string(),
            symbol: TOKEN_SYMBOL.to_string(),
            decimals: 6,
            initial_balances: vec![],
            mint: Some(MinterResponse {
                minter: env.contract.address.to_string(),
                cap: None,
            }),
            init_hook: Some(InitHook {
                msg: to_binary(&ExecuteMsg::PostInitialize {})?,
                contract_addr: env.contract.address.to_string(),
            }),
        })?,
        funds: vec![],
        label: String::from("Astroport Staking Token"),
    }));

    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::PostInitialize {} => try_post_initialize(deps, env, info),
        ExecuteMsg::Enter { amount } => try_enter(&deps, env, info, amount),
        ExecuteMsg::Leave { share } => try_leave(&deps, env, info, share),
    }
}

pub fn try_post_initialize(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if config.share_token_addr != Addr::unchecked("") {
        return Err(ContractError::Unauthorized {});
    }

    // Set token addr
    config.share_token_addr = info.sender;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
}

pub fn try_enter(
    deps: &DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    let total_deposit = get_total_deposit(deps, env.clone(), config.clone())?;
    let total_shares = get_total_shares(deps, config.clone())?;

    // If no balance exists, mint it 1:1 to the amount put in
    let mint_amount: Uint128 = if total_shares.is_zero() || total_deposit.is_zero() {
        amount
    } else {
        amount
            .checked_mul(total_shares)?
            .checked_div(total_deposit)
            .map_err(|e| StdError::DivideByZero { source: e })?
    };

    let res = Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.share_token_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: info.sender.to_string(),
                amount: mint_amount,
            })?,
            funds: vec![],
        }))
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.deposit_token_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: env.contract.address.to_string(),
                amount,
            })?,
            funds: vec![],
        }));

    Ok(res)
}

pub fn try_leave(
    deps: &DepsMut,
    env: Env,
    info: MessageInfo,
    share: Uint128,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    let total_deposit = get_total_deposit(deps, env, config.clone())?;
    let total_shares = get_total_shares(deps, config.clone())?;

    let what = share
        .checked_mul(total_deposit)?
        .checked_div(total_shares)
        .map_err(|e| StdError::DivideByZero { source: e })?;

    // Burn share
    let res = Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.share_token_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::BurnFrom {
                owner: info.sender.to_string(),
                amount: share,
            })?,
            funds: vec![],
        }))
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.deposit_token_addr.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount: what,
            })?,
            funds: vec![],
        }));

    Ok(res)
}

pub fn get_total_shares(deps: &DepsMut, config: Config) -> StdResult<Uint128> {
    let result: TokenInfoResponse = deps
        .querier
        .query_wasm_smart(&config.share_token_addr, &Cw20QueryMsg::TokenInfo {})?;

    Ok(result.total_supply)
}

pub fn get_total_deposit(deps: &DepsMut, env: Env, config: Config) -> StdResult<Uint128> {
    let result: BalanceResponse = deps.querier.query_wasm_smart(
        &config.deposit_token_addr,
        &Cw20QueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;
    Ok(result.balance)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    match msg {
        QueryMsg::Config {} => Ok(to_binary(&ConfigResponse {
            deposit_token_addr: config.deposit_token_addr,
            share_token_addr: config.share_token_addr,
        })?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
