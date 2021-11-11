use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order,
    Reply, ReplyOn, Response, StdError, StdResult, SubMsg, WasmMsg,
};

use crate::error::ContractError;
use crate::querier::query_pair_info;
use crate::state::{
    pair_key, read_pairs, Config, TmpPairInfo, CONFIG, PAIRS, PAIR_CONFIGS, TMP_PAIR_INFO,
};

use crate::response::MsgInstantiateContractResponse;

use astroport::asset::{AssetInfo, PairInfo};
use astroport::factory::{
    ConfigResponse, ExecuteMsg, FeeInfoResponse, InstantiateMsg, MigrateMsg, PairConfig, PairType,
    PairsResponse, QueryMsg,
};
use astroport::hook::InitHook;
use astroport::pair::{
    InstantiateMsg as PairInstantiateMsg, InstantiateMsgStable as PairInstantiateMsgStable,
};
use cw2::set_contract_version;
use protobuf::Message;
use std::collections::HashSet;

// version info for migration info
const CONTRACT_NAME: &str = "astroport-factory";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_PAIR_REPLY_ID: u64 = 1;
const INSTANTIATE_PAIR_STABLE_REPLY_ID: u64 = 2;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let owner = deps.api.addr_validate(&msg.owner)?;

    let generator_address = deps.api.addr_validate(msg.generator_address.as_str())?;
    let mut config = Config {
        owner,
        gov: None,
        token_code_id: msg.token_code_id,
        fee_address: None,
        generator_address,
    };

    if let Some(fee_address) = msg.fee_address {
        config.fee_address = Some(deps.api.addr_validate(fee_address.as_str())?);
    }

    if let Some(gov) = msg.gov {
        config.gov = Some(deps.api.addr_validate(gov.as_str())?);
    }

    let config_set: HashSet<String> = msg
        .pair_configs
        .clone()
        .into_iter()
        .map(|pc| pc.pair_type.to_string())
        .collect();

    if config_set.len() != msg.pair_configs.len() {
        return Err(ContractError::PairConfigDuplicate {});
    }

    for pc in msg.pair_configs.iter() {
        // validate total and maker fee bps
        if !pc.valid_fee_bps() {
            return Err(ContractError::PairConfigInvalidFeeBps {});
        }
        PAIR_CONFIGS.save(deps.storage, pc.clone().pair_type.to_string(), pc)?;
    }

    CONFIG.save(deps.storage, &config)?;

    if let Some(hook) = msg.init_hook {
        Ok(
            Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: hook.contract_addr.to_string(),
                msg: hook.msg,
                funds: vec![],
            })),
        )
    } else {
        Ok(Response::new())
    }
}

pub struct UpdateConfig {
    gov: Option<Addr>,
    owner: Option<Addr>,
    token_code_id: Option<u64>,
    fee_address: Option<Addr>,
    generator_address: Option<Addr>,
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            gov,
            owner,
            token_code_id,
            fee_address,
            generator_address,
        } => execute_update_config(
            deps,
            env,
            info,
            UpdateConfig {
                gov,
                owner,
                token_code_id,
                fee_address,
                generator_address,
            },
        ),
        ExecuteMsg::UpdatePairConfig { config } => execute_update_pair_config(deps, info, config),
        ExecuteMsg::RemovePairConfig { pair_type } => {
            execute_remove_pair_config(deps, info, pair_type)
        }
        ExecuteMsg::CreatePair {
            asset_infos,
            init_hook,
        } => execute_create_pair(deps, env, asset_infos, init_hook),
        ExecuteMsg::CreatePairStable {
            asset_infos,
            init_hook,
            amp,
        } => execute_create_pair_stable(deps, env, asset_infos, amp, init_hook),
        ExecuteMsg::Deregister { asset_infos } => deregister(deps, info, asset_infos),
    }
}

// Only owner can execute it
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    param: UpdateConfig,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(gov) = param.gov {
        // validate address format
        config.gov = Some(deps.api.addr_validate(gov.as_str())?);
    }

    if let Some(owner) = param.owner {
        // validate address format
        config.owner = deps.api.addr_validate(owner.as_str())?;
    }

    if let Some(fee_address) = param.fee_address {
        // validate address format
        config.fee_address = Some(deps.api.addr_validate(fee_address.as_str())?);
    }

    if let Some(generator_address) = param.generator_address {
        // validate address format
        config.generator_address = deps.api.addr_validate(generator_address.as_str())?;
    }

    if let Some(token_code_id) = param.token_code_id {
        config.token_code_id = token_code_id;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

pub fn execute_update_pair_config(
    deps: DepsMut,
    info: MessageInfo,
    pair_config: PairConfig,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // validate total and maker fee bps
    if !pair_config.valid_fee_bps() {
        return Err(ContractError::PairConfigInvalidFeeBps {});
    }

    PAIR_CONFIGS.save(
        deps.storage,
        pair_config.pair_type.to_string(),
        &pair_config,
    )?;

    Ok(Response::new().add_attribute("action", "update_pair_config"))
}

pub fn execute_remove_pair_config(
    deps: DepsMut,
    info: MessageInfo,
    pair_type: PairType,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if !PAIR_CONFIGS.has(deps.storage, pair_type.to_string()) {
        return Err(ContractError::PairConfigNotFound {});
    }

    PAIR_CONFIGS.remove(deps.storage, pair_type.to_string());

    Ok(Response::new().add_attribute("action", "remove_pair_config"))
}

// Anyone can execute it to create swap pair
pub fn execute_create_pair(
    deps: DepsMut,
    env: Env,
    asset_infos: [AssetInfo; 2],
    init_hook: Option<InitHook>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if PAIRS
        .may_load(deps.storage, &pair_key(&asset_infos))?
        .is_some()
    {
        return Err(ContractError::PairWasCreated {});
    }

    let pair_type = PairType::Xyk {};

    // Get pair type from config
    let pair_config = PAIR_CONFIGS
        .load(deps.storage, pair_type.to_string())
        .map_err(|_| ContractError::PairConfigNotFound {})?;

    let pair_key = pair_key(&asset_infos);
    TMP_PAIR_INFO.save(
        deps.storage,
        &TmpPairInfo {
            pair_key,
            owner: env.contract.address.clone(), // TODO: is the factory is the owner of the pair?
        },
    )?;

    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_PAIR_REPLY_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.owner.to_string()),
            code_id: pair_config.code_id,
            msg: to_binary(&PairInstantiateMsg {
                asset_infos: asset_infos.clone(),
                token_code_id: config.token_code_id,
                init_hook: None,
                factory_addr: env.contract.address,
            })?,
            funds: vec![],
            label: "Astroport pair".to_string(),
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    let mut messages: Vec<CosmosMsg> = vec![];
    if let Some(hook) = init_hook {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: hook.contract_addr.to_string(),
            msg: hook.msg,
            funds: vec![],
        }));
    }

    Ok(Response::new()
        .add_submessages(sub_msg)
        .add_messages(messages)
        .add_attributes(vec![
            attr("action", "create_pair"),
            attr("pair", format!("{}-{}", asset_infos[0], asset_infos[1])),
        ]))
}

// Anyone can execute it to create swap stable pair
pub fn execute_create_pair_stable(
    deps: DepsMut,
    env: Env,
    asset_infos: [AssetInfo; 2],
    amp: u64,
    init_hook: Option<InitHook>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if PAIRS
        .may_load(deps.storage, &pair_key(&asset_infos))?
        .is_some()
    {
        return Err(ContractError::PairWasCreated {});
    }

    let pair_type = PairType::Stable {};

    // Get pair type from config
    let pair_config = PAIR_CONFIGS
        .load(deps.storage, pair_type.to_string())
        .map_err(|_| ContractError::PairConfigNotFound {})?;

    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_PAIR_STABLE_REPLY_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.owner.to_string()),
            code_id: pair_config.code_id,
            msg: to_binary(&PairInstantiateMsgStable {
                asset_infos: asset_infos.clone(),
                token_code_id: config.token_code_id,
                init_hook: None,
                factory_addr: env.contract.address,
                amp,
            })?,
            funds: vec![],
            label: "Astroport pair".to_string(),
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    let mut messages: Vec<CosmosMsg> = vec![];
    if let Some(hook) = init_hook {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: hook.contract_addr.to_string(),
            msg: hook.msg,
            funds: vec![],
        }));
    }

    Ok(Response::new()
        .add_submessages(sub_msg)
        .add_messages(messages)
        .add_attributes(vec![
            attr("action", "create_pair_stable"),
            attr("pair", format!("{}-{}", asset_infos[0], asset_infos[1])),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    let tmp = TMP_PAIR_INFO.load(deps.storage)?;
    if PAIRS.may_load(deps.storage, &tmp.pair_key)?.is_some() {
        return Err(ContractError::PairWasRegistered {});
    }

    let data = msg.result.unwrap().data.unwrap();
    let res: MsgInstantiateContractResponse =
        Message::parse_from_bytes(data.as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;

    let pair_contract = deps.api.addr_validate(res.get_contract_address())?;

    PAIRS.save(deps.storage, &tmp.pair_key, &pair_contract)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "register"),
        attr("pair_contract_addr", pair_contract),
    ]))
}

/// create pair executes this message
pub fn deregister(
    deps: DepsMut,
    info: MessageInfo,
    asset_infos: [AssetInfo; 2],
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let pair_addr: Addr = PAIRS.load(deps.storage, &pair_key(&asset_infos))?;
    PAIRS.remove(deps.storage, &pair_key(&asset_infos));

    Ok(Response::new().add_attributes(vec![
        attr("action", "deregister"),
        attr("pair_contract_addr", pair_addr),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Pair { asset_infos } => to_binary(&query_pair(deps, asset_infos)?),
        QueryMsg::Pairs { start_after, limit } => {
            to_binary(&query_pairs(deps, start_after, limit)?)
        }
        QueryMsg::FeeInfo { pair_type } => to_binary(&query_fee_info(deps, pair_type)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let resp = ConfigResponse {
        owner: config.owner,
        gov: config.gov,
        token_code_id: config.token_code_id,
        pair_configs: PAIR_CONFIGS
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| {
                let (_, cfg) = item.unwrap();
                cfg
            })
            .collect(),
        fee_address: config.fee_address,
        generator_address: config.generator_address,
    };

    Ok(resp)
}

pub fn query_pair(deps: Deps, asset_infos: [AssetInfo; 2]) -> StdResult<PairInfo> {
    let pair_addr = PAIRS.load(deps.storage, &pair_key(&asset_infos))?;
    query_pair_info(deps, &pair_addr)
}

pub fn query_pairs(
    deps: Deps,
    start_after: Option<[AssetInfo; 2]>,
    limit: Option<u32>,
) -> StdResult<PairsResponse> {
    let pairs: Vec<PairInfo> = read_pairs(deps, start_after, limit)
        .iter()
        .map(|pair_addr| query_pair_info(deps, pair_addr).unwrap())
        .collect();

    Ok(PairsResponse { pairs })
}

pub fn query_fee_info(deps: Deps, pair_type: PairType) -> StdResult<FeeInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let pair_config = PAIR_CONFIGS.load(deps.storage, pair_type.to_string())?;

    Ok(FeeInfoResponse {
        fee_address: config.fee_address,
        total_fee_bps: pair_config.total_fee_bps,
        maker_fee_bps: pair_config.maker_fee_bps,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
