use astroport::staking::{ConfigResponse, Cw20HookMsg, InstantiateMsg as xInstatiateMsg, QueryMsg};
use astroport::token::InstantiateMsg;
use cosmwasm_std::{
    attr,
    testing::{mock_env, MockApi, MockStorage},
    to_binary, Addr, QueryRequest, Uint128, WasmQuery,
};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};
use terra_multi_test::{AppBuilder, BankKeeper, ContractWrapper, Executor, TerraApp, TerraMock};

const ALICE: &str = "Alice";
const BOB: &str = "Bob";
const CAROL: &str = "Carol";

fn mock_app() -> TerraApp {
    let env = mock_env();
    let api = MockApi::default();
    let bank = BankKeeper::new();
    let storage = MockStorage::new();
    let custom = TerraMock::luna_ust_case();

    AppBuilder::new()
        .with_api(api)
        .with_block(env.block)
        .with_bank(bank)
        .with_storage(storage)
        .with_custom(custom)
        .build()
}

fn instantiate_contracts(router: &mut TerraApp, owner: Addr) -> (Addr, Addr, Addr) {
    let astro_token_contract = Box::new(ContractWrapper::new_with_empty(
        astroport_token::contract::execute,
        astroport_token::contract::instantiate,
        astroport_token::contract::query,
    ));

    let astro_token_code_id = router.store_code(astro_token_contract);

    let msg = InstantiateMsg {
        name: String::from("Astro token"),
        symbol: String::from("ASTRO"),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
    };

    let astro_token_instance = router
        .instantiate_contract(
            astro_token_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("ASTRO"),
            None,
        )
        .unwrap();

    let staking_contract = Box::new(
        ContractWrapper::new_with_empty(
            astroport_staking::contract::execute,
            astroport_staking::contract::instantiate,
            astroport_staking::contract::query,
        )
        .with_reply_empty(astroport_staking::contract::reply),
    );

    let staking_code_id = router.store_code(staking_contract);

    let msg = xInstatiateMsg {
        owner: owner.to_string(),
        token_code_id: astro_token_code_id,
        deposit_token_addr: astro_token_instance.to_string(),
    };
    let staking_instance = router
        .instantiate_contract(
            staking_code_id,
            owner,
            &msg,
            &[],
            String::from("xASTRO"),
            None,
        )
        .unwrap();

    let msg = QueryMsg::Config {};
    let res = router
        .wrap()
        .query::<ConfigResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: staking_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }))
        .unwrap();

    // in multitest, contract names are named in the order in which contracts are created.
    assert_eq!("contract #0", astro_token_instance);
    assert_eq!("contract #1", staking_instance);
    assert_eq!("contract #2", res.share_token_addr);

    let x_astro_token_instance = res.share_token_addr;

    (
        astro_token_instance,
        staking_instance,
        x_astro_token_instance,
    )
}

fn mint_some_astro(router: &mut TerraApp, owner: Addr, astro_token_instance: Addr, to: &str) {
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: String::from(to),
        amount: Uint128::from(100u128),
    };
    let res = router
        .execute_contract(owner.clone(), astro_token_instance.clone(), &msg, &[])
        .unwrap();
    assert_eq!(res.events[1].attributes[1], attr("action", "mint"));
    assert_eq!(res.events[1].attributes[2], attr("to", String::from(to)));
    assert_eq!(
        res.events[1].attributes[3],
        attr("amount", Uint128::from(100u128))
    );
}

#[test]
fn cw20receive_enter_and_leave() {
    let mut router = mock_app();

    let owner = Addr::unchecked("owner");

    let (astro_token_instance, staking_instance, x_astro_token_instance) =
        instantiate_contracts(&mut router, owner.clone());

    // mint 100 ASTRO for Alice
    mint_some_astro(
        &mut router,
        owner.clone(),
        astro_token_instance.clone(),
        ALICE,
    );

    let alice_address = Addr::unchecked(ALICE);

    // check if Alice's ASTRO balance is 100
    let msg = Cw20QueryMsg::Balance {
        address: alice_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(100u128)
        }
    );

    // we can leave tokens only from xAstro token.
    let msg = Cw20ExecuteMsg::Send {
        contract: staking_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::Leave {}).unwrap(),
        amount: Uint128::from(10u128),
    };

    let resp = router
        .execute_contract(
            alice_address.clone(),
            astro_token_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(resp.to_string(), "Unauthorized");

    // try to enter Alice's 100 ASTRO for 100 xASTRO
    let msg = Cw20ExecuteMsg::Send {
        contract: staking_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::Enter {}).unwrap(),
        amount: Uint128::from(100u128),
    };

    router
        .execute_contract(
            alice_address.clone(),
            astro_token_instance.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // check if Alice's xASTRO balance is 100
    let msg = Cw20QueryMsg::Balance {
        address: alice_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(100u128)
        }
    );

    // check if Alice's ASTRO balance is 0
    let msg = Cw20QueryMsg::Balance {
        address: alice_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(0u128)
        }
    );

    // check if staking contract's ASTRO balance is 100
    let msg = Cw20QueryMsg::Balance {
        address: staking_instance.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(100u128)
        }
    );

    // we can enter tokens only from Astro token.
    let msg = Cw20ExecuteMsg::Send {
        contract: staking_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::Enter {}).unwrap(),
        amount: Uint128::from(10u128),
    };

    let resp = router
        .execute_contract(
            alice_address.clone(),
            x_astro_token_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(resp.to_string(), "Unauthorized");

    // try to leave Alice's 10 xASTRO for 10 ASTRO
    let msg = Cw20ExecuteMsg::Send {
        contract: staking_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::Leave {}).unwrap(),
        amount: Uint128::from(10u128),
    };

    router
        .execute_contract(
            alice_address.clone(),
            x_astro_token_instance.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // check if Alice's xASTRO balance is 90
    let msg = Cw20QueryMsg::Balance {
        address: alice_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(90u128)
        }
    );

    // check if Alice's ASTRO balance is 10
    let msg = Cw20QueryMsg::Balance {
        address: alice_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(10u128)
        }
    );

    // check if staking contract's ASTRO balance is 90
    let msg = Cw20QueryMsg::Balance {
        address: staking_instance.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(90u128)
        }
    );

    // check if staking contract's xASTRO balance is 0
    let msg = Cw20QueryMsg::Balance {
        address: staking_instance.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(0u128)
        }
    );
}

#[test]
fn should_not_allow_withdraw_more_than_what_you_have() {
    let mut router = mock_app();

    let owner = Addr::unchecked("owner");

    let (astro_token_instance, staking_instance, x_astro_token_instance) =
        instantiate_contracts(&mut router, owner.clone());

    // mint 100 ASTRO for Alice
    mint_some_astro(
        &mut router,
        owner.clone(),
        astro_token_instance.clone(),
        ALICE,
    );
    let alice_address = Addr::unchecked(ALICE);

    // enter Alice's 100 ASTRO for 100 xASTRO
    let msg = Cw20ExecuteMsg::Send {
        contract: staking_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::Enter {}).unwrap(),
        amount: Uint128::from(100u128),
    };

    router
        .execute_contract(
            alice_address.clone(),
            astro_token_instance.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // check if Alice's xASTRO balance is 100
    let msg = Cw20QueryMsg::Balance {
        address: alice_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(100u128)
        }
    );

    // try to leave Alice's 200 xASTRO
    let msg = Cw20ExecuteMsg::Send {
        contract: staking_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::Leave {}).unwrap(),
        amount: Uint128::from(200u128),
    };

    let res = router
        .execute_contract(
            alice_address.clone(),
            x_astro_token_instance.clone(),
            &msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(res.to_string(), "Overflow: Cannot Sub with 100 and 200");
}

#[test]
fn should_work_with_more_than_one_participant() {
    let mut router = mock_app();

    let owner = Addr::unchecked("owner");

    let (astro_token_instance, staking_instance, x_astro_token_instance) =
        instantiate_contracts(&mut router, owner.clone());

    // mint 100 ASTRO for Alice
    mint_some_astro(
        &mut router,
        owner.clone(),
        astro_token_instance.clone(),
        ALICE,
    );
    let alice_address = Addr::unchecked(ALICE);

    // mint 100 ASTRO for Bob
    mint_some_astro(
        &mut router,
        owner.clone(),
        astro_token_instance.clone(),
        BOB,
    );
    let bob_address = Addr::unchecked(BOB);

    // mint 100 ASTRO for Carol
    mint_some_astro(
        &mut router,
        owner.clone(),
        astro_token_instance.clone(),
        CAROL,
    );
    let carol_address = Addr::unchecked(CAROL);

    // enter Alice's 20 ASTRO for 20 xASTRO
    let msg = Cw20ExecuteMsg::Send {
        contract: staking_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::Enter {}).unwrap(),
        amount: Uint128::from(20u128),
    };

    router
        .execute_contract(
            alice_address.clone(),
            astro_token_instance.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // enter Bob's 10 ASTRO for 10 xASTRO
    let msg = Cw20ExecuteMsg::Send {
        contract: staking_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::Enter {}).unwrap(),
        amount: Uint128::from(10u128),
    };

    router
        .execute_contract(bob_address.clone(), astro_token_instance.clone(), &msg, &[])
        .unwrap();

    // check if Alice's xASTRO balance is 20
    let msg = Cw20QueryMsg::Balance {
        address: alice_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(20u128)
        }
    );

    // check if Bob's xASTRO balance is 10
    let msg = Cw20QueryMsg::Balance {
        address: bob_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(10u128)
        }
    );

    // check if staking contract's ASTRO balance is 30
    let msg = Cw20QueryMsg::Balance {
        address: staking_instance.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(30u128)
        }
    );

    // staking contract gets 20 more ASTRO from external source
    let msg = Cw20ExecuteMsg::Transfer {
        recipient: staking_instance.to_string(),
        amount: Uint128::from(20u128),
    };
    let res = router
        .execute_contract(
            carol_address.clone(),
            astro_token_instance.clone(),
            &msg,
            &[],
        )
        .unwrap();
    assert_eq!(res.events[1].attributes[1], attr("action", "transfer"));
    assert_eq!(res.events[1].attributes[2], attr("from", carol_address));
    assert_eq!(
        res.events[1].attributes[3],
        attr("to", staking_instance.clone())
    );
    assert_eq!(
        res.events[1].attributes[4],
        attr("amount", Uint128::from(20u128))
    );

    // enter Alice's 10 ASTRO for 6 xASTRO: 10*30/50 = 6
    let msg = Cw20ExecuteMsg::Send {
        contract: staking_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::Enter {}).unwrap(),
        amount: Uint128::from(10u128),
    };

    router
        .execute_contract(
            alice_address.clone(),
            astro_token_instance.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // check if Alice's xASTRO balance is 26
    let msg = Cw20QueryMsg::Balance {
        address: alice_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(26u128)
        }
    );

    // check if Bob's xASTRO balance is 10
    let msg = Cw20QueryMsg::Balance {
        address: bob_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(10u128)
        }
    );

    // leave Bob's 5 xASTRO: gets 5*60/36 = 8 ASTRO
    let msg = Cw20ExecuteMsg::Send {
        contract: staking_instance.to_string(),
        msg: to_binary(&Cw20HookMsg::Leave {}).unwrap(),
        amount: Uint128::from(5u128),
    };

    router
        .execute_contract(
            bob_address.clone(),
            x_astro_token_instance.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // check if Alice's xASTRO balance is 26
    let msg = Cw20QueryMsg::Balance {
        address: alice_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(26u128)
        }
    );

    // check if Bob's xASTRO balance is 5
    let msg = Cw20QueryMsg::Balance {
        address: bob_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: x_astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(5u128)
        }
    );

    // check if staking contract's ASTRO balance is 52 (60 - 8 (Bob left 5 xASTRO))
    let msg = Cw20QueryMsg::Balance {
        address: staking_instance.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(52u128)
        }
    );

    // check if Alice's ASTRO balance is 70 (100 minted - 20 entered - 10 entered)
    let msg = Cw20QueryMsg::Balance {
        address: alice_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(70u128)
        }
    );

    // check if Bob's ASTRO balance is 98 (100 minted - 10 entered + 8 by leaving)
    let msg = Cw20QueryMsg::Balance {
        address: bob_address.to_string(),
    };
    let res: Result<BalanceResponse, _> =
        router.wrap().query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: astro_token_instance.to_string(),
            msg: to_binary(&msg).unwrap(),
        }));
    assert_eq!(
        res.unwrap(),
        BalanceResponse {
            balance: Uint128::from(98u128)
        }
    );
}
