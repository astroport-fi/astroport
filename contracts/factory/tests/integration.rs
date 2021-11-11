use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{attr, Addr};

use astroport::asset::{AssetInfo, PairInfo};
use astroport::factory::{ConfigResponse, ExecuteMsg, InstantiateMsg, PairConfig, QueryMsg};

use terra_multi_test::{App, BankKeeper, ContractWrapper, Executor, TerraMockQuerier};

fn mock_app() -> App {
    let api = MockApi::default();
    let env = mock_env();
    let bank = BankKeeper::new();
    let storage = MockStorage::new();
    let tmq = TerraMockQuerier::new(MockQuerier::new(&[]));

    App::new(api, env.block, bank, storage, tmq)
}

fn store_factory_code(app: &mut App) -> u64 {
    let factory_contract = Box::new(
        ContractWrapper::new(
            astroport_factory::contract::execute,
            astroport_factory::contract::instantiate,
            astroport_factory::contract::query,
        )
        .with_reply(astroport_factory::contract::reply),
    );

    app.store_code(factory_contract)
}

fn store_pair_code(app: &mut App) -> u64 {
    let pair_contract = Box::new(
        ContractWrapper::new(
            astroport_pair::contract::execute,
            astroport_pair::contract::instantiate,
            astroport_pair::contract::query,
        )
        .with_reply(astroport_pair::contract::reply),
    );

    app.store_code(pair_contract)
}

fn store_token_code(app: &mut App) -> u64 {
    let token_contract = Box::new(ContractWrapper::new(
        astroport_token::contract::execute,
        astroport_token::contract::instantiate,
        astroport_token::contract::query,
    ));

    app.store_code(token_contract)
}

#[test]
fn proper_initialization() {
    let mut app = mock_app();

    let owner = Addr::unchecked("Owner");

    let factory_code_id = store_factory_code(&mut app);

    let pair_config = PairConfig {
        code_id: 321,
        total_fee_bps: 100,
        maker_fee_bps: 10,
    };

    let msg = InstantiateMsg {
        pair_xyk_config: Some(pair_config.clone()),
        pair_stable_config: None,
        token_code_id: 123,
        init_hook: None,
        fee_address: None,
        gov: None,
        owner: owner.to_string(),
        generator_address: Addr::unchecked("generator"),
    };

    let factory_instance = app
        .instantiate_contract(factory_code_id, owner.clone(), &msg, &[], "factory", None)
        .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&factory_instance, &msg)
        .unwrap();

    assert_eq!(123, config_res.token_code_id);
    assert_eq!(Some(pair_config), config_res.pair_xyk_config);
    assert_eq!(None, config_res.pair_stable_config);
    assert_eq!(owner, config_res.owner);
}

#[test]
fn update_config() {
    let mut app = mock_app();

    let owner = Addr::unchecked("Owner");
    let new_owner = Addr::unchecked("NewOnwer");

    let token_code_id = store_token_code(&mut app);
    let factory_instance = instantiate_contract(&mut app, &owner, token_code_id);

    let pair_config = PairConfig {
        code_id: 321,
        total_fee_bps: 100,
        maker_fee_bps: 10,
    };

    // update owner
    let msg = ExecuteMsg::UpdateConfig {
        gov: Some(new_owner.clone()),
        owner: Some(new_owner.clone()),
        token_code_id: None,
        fee_address: None,
        generator_address: None,
        pair_xyk_config: None,
        pair_stable_config: Some(pair_config.clone()),
    };

    app.execute_contract(owner.clone(), factory_instance.clone(), &msg, &[])
        .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&factory_instance, &msg)
        .unwrap();
    assert_eq!(token_code_id, config_res.token_code_id);
    assert_eq!(new_owner.clone(), config_res.owner);
    assert_eq!(Some(pair_config), config_res.pair_stable_config);

    // update left items
    let fee_address = Some(Addr::unchecked("fee"));
    let msg = ExecuteMsg::UpdateConfig {
        gov: None,
        owner: None,
        token_code_id: Some(200u64),
        fee_address: fee_address.clone(),
        generator_address: None,
        pair_xyk_config: None,
        pair_stable_config: None,
    };

    app.execute_contract(new_owner, factory_instance.clone(), &msg, &[])
        .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&factory_instance, &msg)
        .unwrap();
    assert_eq!(200u64, config_res.token_code_id);
    assert_eq!(fee_address, config_res.fee_address);

    // Unauthorized err
    let msg = ExecuteMsg::UpdateConfig {
        gov: None,
        owner: None,
        token_code_id: None,
        fee_address: None,
        generator_address: None,
        pair_xyk_config: None,
        pair_stable_config: None,
    };

    let res = app
        .execute_contract(owner, factory_instance, &msg, &[])
        .unwrap_err();
    assert_eq!(res.to_string(), "Unauthorized");
}

fn instantiate_contract(app: &mut App, owner: &Addr, token_code_id: u64) -> Addr {
    let pair_code_id = store_pair_code(app);
    let factory_code_id = store_factory_code(app);

    let pair_config = PairConfig {
        code_id: pair_code_id,
        total_fee_bps: 100,
        maker_fee_bps: 10,
    };

    let msg = InstantiateMsg {
        pair_xyk_config: Some(pair_config),
        pair_stable_config: None,
        token_code_id,
        init_hook: None,
        fee_address: None,
        gov: None,
        owner: owner.to_string(),
        generator_address: Addr::unchecked("generator"),
    };

    app.instantiate_contract(
        factory_code_id,
        owner.to_owned(),
        &msg,
        &[],
        "factory",
        None,
    )
    .unwrap()
}

#[test]
fn create_pair() {
    let mut app = mock_app();

    let owner = Addr::unchecked("Owner");

    let token_code_id = store_token_code(&mut app);

    let factory_instance = instantiate_contract(&mut app, &owner, token_code_id);

    let asset_infos = [
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0000"),
        },
        AssetInfo::Token {
            contract_addr: Addr::unchecked("asset0001"),
        },
    ];
    let msg = ExecuteMsg::CreatePair {
        asset_infos: asset_infos.clone(),
        init_hook: None,
    };

    let res = app
        .execute_contract(owner, factory_instance.clone(), &msg, &[])
        .unwrap();

    assert_eq!(res.events[1].attributes[1], attr("action", "create_pair"));
    assert_eq!(
        res.events[1].attributes[2],
        attr("pair", "asset0000-asset0001")
    );

    let res: PairInfo = app
        .wrap()
        .query_wasm_smart(
            factory_instance.clone(),
            &QueryMsg::Pair {
                asset_infos: asset_infos.clone(),
            },
        )
        .unwrap();
    assert_eq!("Contract #0", factory_instance.to_string());
    assert_eq!("Contract #1", res.contract_addr.to_string());
    assert_eq!("Contract #2", res.liquidity_token.to_string());
}
