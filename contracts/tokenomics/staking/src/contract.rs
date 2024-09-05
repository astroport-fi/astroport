#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, coin, ensure, to_json_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Reply, Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw_utils::{must_pay, parse_reply_instantiate_data, MsgInstantiateContractResponse};
use osmosis_std::types::cosmos::bank::v1beta1::{DenomUnit, Metadata};
use osmosis_std::types::osmosis::tokenfactory::v1beta1::{
    MsgBurn, MsgCreateDenom, MsgCreateDenomResponse, MsgMint, MsgSetBeforeSendHook,
    MsgSetDenomMetadata,
};

use astroport::staking::{
    Config, ExecuteMsg, InstantiateMsg, QueryMsg, StakingResponse, TrackerData,
};

use crate::error::ContractError;
use crate::state::{CONFIG, TRACKER_DATA};

/// Contract name that is used for migration.
pub const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
/// Contract version that is used for migration.
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// xASTRO information
const TOKEN_NAME: &str = "Staked Astroport Token";
const TOKEN_SYMBOL: &str = "xASTRO";

/// A `reply` call code ID used for sub-messages.
enum ReplyIds {
    InstantiateDenom = 1,
    InstantiateTrackingContract = 2,
}

impl TryFrom<u64> for ReplyIds {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(ReplyIds::InstantiateDenom),
            2 => Ok(ReplyIds::InstantiateTrackingContract),
            _ => Err(ContractError::FailedToParseReply {}),
        }
    }
}

/// Minimum initial xastro share
pub(crate) const MINIMUM_STAKE_AMOUNT: Uint128 = Uint128::new(1_000);

/// Creates a new contract with the specified parameters in the [`InstantiateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Validate that deposit_token_denom exists on chain
    let supply = deps.querier.query_supply(&msg.deposit_token_denom)?.amount;
    ensure!(
        !supply.is_zero(),
        StdError::generic_err(
            "deposit_token_denom has 0 supply which is likely sign of misconfiguration"
        )
    );

    // Validate addresses
    deps.api.addr_validate(&msg.token_factory_addr)?;
    deps.api.addr_validate(&msg.tracking_admin)?;

    CONFIG.save(
        deps.storage,
        &Config {
            astro_denom: msg.deposit_token_denom,
            xastro_denom: "".to_string(),
        },
    )?;

    // Store tracker data
    TRACKER_DATA.save(
        deps.storage,
        &TrackerData {
            code_id: msg.tracking_code_id,
            admin: msg.tracking_admin,
            token_factory_addr: msg.token_factory_addr,
            tracker_addr: "".to_string(),
        },
    )?;

    let create_denom_msg = SubMsg::reply_on_success(
        MsgCreateDenom {
            sender: env.contract.address.to_string(),
            subdenom: TOKEN_SYMBOL.to_owned(),
        },
        ReplyIds::InstantiateDenom as u64,
    );

    Ok(Response::new().add_submessage(create_denom_msg))
}

/// Exposes execute functions available in the contract.
///
/// ## Variants
/// * **ExecuteMsg::Enter** Stake the provided ASTRO tokens for xASTRO
/// * **ExecuteMsg::Leave** Unstake the provided xASTRO tokens for ASTRO
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Enter { receiver } => {
            // xASTRO is minted to the receiver if provided or to the sender.
            let recipient = receiver.unwrap_or_else(|| info.sender.to_string());
            execute_enter(deps, env, info).map(|(resp, minted_coins)| {
                resp.add_message(BankMsg::Send {
                    to_address: recipient.clone(),
                    amount: vec![minted_coins],
                })
                .add_attributes([("action", "enter"), ("recipient", recipient.as_str())])
            })
        }
        ExecuteMsg::EnterWithHook {
            contract_address,
            msg,
        } => execute_enter(deps, env, info).map(|(resp, minted_coins)| {
            resp.add_message(WasmMsg::Execute {
                contract_addr: contract_address.clone(),
                msg,
                funds: vec![minted_coins],
            })
            .add_attributes([
                ("action", "enter_with_hook"),
                ("next_contract", &contract_address),
            ])
        }),
        ExecuteMsg::Leave {} => execute_leave(deps, env, info),
    }
}

/// The entry point to the contract for processing replies from submessages.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match ReplyIds::try_from(msg.id)? {
        ReplyIds::InstantiateDenom => {
            let MsgCreateDenomResponse { new_token_denom } = msg.result.try_into()?;

            let denom_metadata_msg = MsgSetDenomMetadata {
                sender: env.contract.address.to_string(),
                metadata: Some(Metadata {
                    symbol: TOKEN_SYMBOL.to_string(),
                    name: TOKEN_NAME.to_string(),
                    base: new_token_denom.clone(),
                    display: TOKEN_SYMBOL.to_string(),
                    denom_units: vec![
                        DenomUnit {
                            denom: new_token_denom.clone(),
                            exponent: 0,
                            aliases: vec![],
                        },
                        DenomUnit {
                            denom: TOKEN_SYMBOL.to_string(),
                            exponent: 6,
                            aliases: vec![],
                        },
                    ],
                    description: "Astroport is a neutral marketplace where anyone, from anywhere in the galaxy, can dock to trade their wares.".to_string(),
                    uri: "https://app.astroport.fi/tokens/xAstro.svg".to_string(),
                    uri_hash: "d39cfe20605a9857b2b123c6d6dbbdf4d3b65cb9d411cee1011877b918b4c646".to_string(),
                }),
            };

            CONFIG.update::<_, StdError>(deps.storage, |mut config| {
                config.xastro_denom = new_token_denom.clone();
                Ok(config)
            })?;

            let tracker_data = TRACKER_DATA.load(deps.storage)?;

            let init_tracking_contract = SubMsg::reply_on_success(
                WasmMsg::Instantiate {
                    admin: Some(tracker_data.admin),
                    code_id: tracker_data.code_id,
                    msg: to_json_binary(&astroport_v4::tokenfactory_tracker::InstantiateMsg {
                        tokenfactory_module_address: tracker_data.token_factory_addr,
                        tracked_denom: new_token_denom.clone(),
                    })?,
                    funds: vec![],
                    label: format!("{TOKEN_SYMBOL} balances tracker"),
                },
                ReplyIds::InstantiateTrackingContract as u64,
            );

            Ok(Response::new()
                .add_submessages([SubMsg::new(denom_metadata_msg), init_tracking_contract])
                .add_attribute("xastro_denom", new_token_denom))
        }
        ReplyIds::InstantiateTrackingContract => {
            let MsgInstantiateContractResponse {
                contract_address, ..
            } = parse_reply_instantiate_data(msg)?;

            TRACKER_DATA.update::<_, StdError>(deps.storage, |mut tracker_data| {
                tracker_data.tracker_addr = contract_address.clone();
                Ok(tracker_data)
            })?;

            let config = CONFIG.load(deps.storage)?;

            // Enable balance tracking for xASTRO
            let set_hook_msg = MsgSetBeforeSendHook {
                sender: env.contract.address.to_string(),
                denom: config.xastro_denom,
                cosmwasm_address: contract_address.clone(),
            };

            Ok(Response::new()
                .add_message(set_hook_msg)
                .add_attribute("tracker_contract", contract_address))
        }
    }
}

/// Enter stakes TokenFactory ASTRO for xASTRO.
/// Returns composed Response object and minted xASTRO in the form of [`Coin`].
/// Subsequent messages are added after,
/// depending on whether it is a plain enter or enter with hook endpoint.
fn execute_enter(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<(Response, Coin), ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Ensure that the correct denom is sent. Sending zero tokens is prohibited on chain level
    let amount = must_pay(&info, &config.astro_denom)?;

    // Get the current deposits and shares held in the contract.
    // Amount sent along with the message already included. Subtract it from the total deposit
    let total_deposit = deps
        .querier
        .query_balance(&env.contract.address, &config.astro_denom)?
        .amount
        - amount;
    let total_shares = deps.querier.query_supply(&config.xastro_denom)?.amount;

    let mut messages: Vec<CosmosMsg> = vec![];

    let mint_amount = if total_shares.is_zero() || total_deposit.is_zero() {
        // There needs to be a minimum amount initially staked, thus the result
        // cannot be zero if the amount is not enough
        if amount.saturating_sub(MINIMUM_STAKE_AMOUNT).is_zero() {
            return Err(ContractError::MinimumStakeAmountError {});
        }

        // Mint the xASTRO tokens to ourselves if this is the first stake
        messages.push(
            MsgMint {
                sender: env.contract.address.to_string(),
                amount: Some(coin(MINIMUM_STAKE_AMOUNT.u128(), &config.xastro_denom).into()),
                mint_to_address: env.contract.address.to_string(),
            }
            .into(),
        );

        amount - MINIMUM_STAKE_AMOUNT
    } else {
        amount.multiply_ratio(total_shares, total_deposit)
    };

    if mint_amount.is_zero() {
        return Err(ContractError::StakeAmountTooSmall {});
    }

    let minted_coins = coin(mint_amount.u128(), config.xastro_denom);

    // Mint new xASTRO tokens to the staking contract
    messages.push(
        MsgMint {
            sender: env.contract.address.to_string(),
            amount: Some(minted_coins.clone().into()),
            mint_to_address: env.contract.address.to_string(),
        }
        .into(),
    );

    // Set the data to be returned in set_data to easy integration with
    // other contracts
    let staking_response = to_json_binary(&StakingResponse {
        astro_amount: amount,
        xastro_amount: mint_amount,
    })?;

    Ok((
        Response::new()
            .add_messages(messages)
            .set_data(staking_response)
            .add_attributes([
                attr("astro_amount", amount),
                attr("xastro_amount", mint_amount),
            ]),
        minted_coins,
    ))
}

/// Leave unstakes TokenFactory xASTRO for ASTRO. xASTRO is burned and ASTRO
/// returned to the sender
fn execute_leave(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Ensure that the correct denom is sent. Sending zero tokens is prohibited on chain level
    let amount = must_pay(&info, &config.xastro_denom)?;

    // Get the current deposits and shares held in the contract
    let total_deposit = deps
        .querier
        .query_balance(&env.contract.address, &config.astro_denom)?
        .amount;
    let total_shares = deps.querier.query_supply(&config.xastro_denom)?.amount;

    // Calculate the amount of ASTRO to return based on the ratios of
    // deposit and shares
    let return_amount = amount.multiply_ratio(total_deposit, total_shares);

    let messages: Vec<CosmosMsg> = vec![
        // Burn the received xASTRO tokens
        MsgBurn {
            sender: env.contract.address.to_string(),
            amount: Some(coin(amount.u128(), &config.xastro_denom).into()),
            burn_from_address: "".to_string(), // This needs to be "" for now
        }
        .into(),
        // Send ASTRO to the sender
        BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![coin(return_amount.u128(), config.astro_denom)],
        }
        .into(),
        // Send xASTRO to itself to trigger total supply snapshot in tracker contract
        BankMsg::Send {
            to_address: env.contract.address.to_string(),
            amount: vec![coin(1, &config.xastro_denom)],
        }
        .into(),
    ];

    // Set the data to be returned in set_data to easy integration with
    // other contracts
    let staking_response = to_json_binary(&StakingResponse {
        astro_amount: return_amount,
        xastro_amount: amount,
    })?;

    Ok(Response::new()
        .add_messages(messages)
        .set_data(staking_response)
        .add_attributes([
            attr("action", "leave"),
            attr("recipient", info.sender),
            attr("xastro_amount", amount),
            attr("astro_amount", return_amount),
        ]))
}

/// Exposes all the queries available in the contract.
///
/// * **QueryMsg::Config {}** Returns the staking contract configuration
///
/// * **QueryMsg::TotalShares {}** Returns the total xASTRO supply
///
/// * **QueryMsg::TotalDeposit {}** Returns the amount of ASTRO that's currently in the staking pool
///
/// * **QueryMsg::TrackerConfig {}** Returns the tracker contract configuration
///
/// * **QueryMsg::BalanceAt { address, timestamp }** Returns the xASTRO balance of the given address at the given timestamp
///
/// * **QueryMsg::TotalSupplyAt { timestamp }** Returns xASTRO total supply at the given timestamp
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::TotalShares {} => {
            let config = CONFIG.load(deps.storage)?;

            let total_supply = deps.querier.query_supply(config.xastro_denom)?.amount;
            to_json_binary(&total_supply)
        }
        QueryMsg::TotalDeposit {} => {
            let config = CONFIG.load(deps.storage)?;

            let total_deposit = deps
                .querier
                .query_balance(env.contract.address, config.astro_denom)?
                .amount;
            to_json_binary(&total_deposit)
        }
        QueryMsg::TrackerConfig {} => to_json_binary(&TRACKER_DATA.load(deps.storage)?),
        QueryMsg::BalanceAt { address, timestamp } => {
            let amount = if timestamp.is_none() {
                let config = CONFIG.load(deps.storage)?;
                deps.querier
                    .query_balance(&address, config.xastro_denom)?
                    .amount
            } else {
                let tracker_config = TRACKER_DATA.load(deps.storage)?;
                deps.querier.query_wasm_smart(
                    tracker_config.tracker_addr,
                    &astroport_v4::tokenfactory_tracker::QueryMsg::BalanceAt { address, timestamp },
                )?
            };

            to_json_binary(&amount)
        }
        QueryMsg::TotalSupplyAt { timestamp } => {
            let amount = if timestamp.is_none() {
                let config = CONFIG.load(deps.storage)?;
                deps.querier.query_supply(config.xastro_denom)?.amount
            } else {
                let tracker_config = TRACKER_DATA.load(deps.storage)?;
                deps.querier.query_wasm_smart(
                    tracker_config.tracker_addr,
                    &astroport_v4::tokenfactory_tracker::QueryMsg::TotalSupplyAt { timestamp },
                )?
            };

            to_json_binary(&amount)
        }
    }
}
