use astroport::asset::{Asset, AssetInfo};
use astroport::pair::ExecuteMsg as PairExecuteMsg;
use astroport::querier::{query_balance, query_pair_info, query_token_balance};
use astroport::router::SwapOperation;
use cosmwasm_std::{
    to_binary, Coin, CosmosMsg, Decimal, DepsMut, Env, MessageInfo, Response, StdResult, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::state::CONFIG;

/// Execute a swap operation.
///
/// * **operation** to perform (native or Astro swap with offer and ask asset information).
///
/// * **to** address that receives the ask assets.
pub fn execute_swap_operation(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    operation: SwapOperation,
    to: Option<String>,
    max_spread: Option<Decimal>,
) -> Result<Response, ContractError> {
    if env.contract.address != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let message = match operation {
        SwapOperation::AstroSwap {
            offer_asset_info,
            ask_asset_info,
        } => {
            let config = CONFIG.load(deps.storage)?;
            let pair_info = query_pair_info(
                &deps.querier,
                &config.astroport_factory,
                &[offer_asset_info.clone(), ask_asset_info.clone()],
            )?;

            let amount = match &offer_asset_info {
                AssetInfo::NativeToken { denom } => {
                    query_balance(&deps.querier, env.contract.address, denom)?
                }
                AssetInfo::Token { contract_addr } => {
                    query_token_balance(&deps.querier, contract_addr, env.contract.address)?
                }
            };
            let offer_asset = Asset {
                info: offer_asset_info,
                amount,
            };

            asset_into_swap_msg(
                deps,
                pair_info.contract_addr.to_string(),
                offer_asset,
                ask_asset_info,
                max_spread,
                to,
            )?
        }
        SwapOperation::NativeSwap { .. } => return Err(ContractError::NativeSwapNotSupported {}),
    };

    Ok(Response::new().add_message(message))
}

/// Creates a message of type [`CosmosMsg`] representing a swap operation.
///
/// * **pair_contract** Astroport pair contract for which the swap operation is performed.
///
/// * **offer_asset** asset that is swapped. It also mentions the amount to swap.
///
/// * **ask_asset_info** asset that is swapped to.
///
/// * **max_spread** max spread enforced for the swap.
///
/// * **to** address that receives the ask assets.
pub fn asset_into_swap_msg(
    deps: DepsMut,
    pair_contract: String,
    offer_asset: Asset,
    ask_asset_info: AssetInfo,
    max_spread: Option<Decimal>,
    to: Option<String>,
) -> StdResult<CosmosMsg> {
    match &offer_asset.info {
        AssetInfo::NativeToken { denom } => {
            // Deduct tax first
            let amount = offer_asset
                .amount
                .checked_sub(offer_asset.compute_tax(&deps.querier)?)?;
            Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pair_contract,
                funds: vec![Coin {
                    denom: denom.to_string(),
                    amount,
                }],
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: Asset {
                        amount,
                        ..offer_asset
                    },
                    ask_asset_info: Some(ask_asset_info),
                    belief_price: None,
                    max_spread,
                    to,
                })?,
            }))
        }
        AssetInfo::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: pair_contract,
                amount: offer_asset.amount,
                msg: to_binary(&astroport::pair::Cw20HookMsg::Swap {
                    ask_asset_info: Some(ask_asset_info),
                    belief_price: None,
                    max_spread,
                    to,
                })?,
            })?,
        })),
    }
}
