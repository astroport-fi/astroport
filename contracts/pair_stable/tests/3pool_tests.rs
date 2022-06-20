use astroport_pair_stable::error::ContractError;
use cosmwasm_std::Addr;

use crate::helper::AssetInfoExt;
use crate::helper::{Helper, TestCoin};

mod helper;

#[test]
fn provide_and_withdraw_no_fee() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::native("uluna"),
        TestCoin::cw20("USDC"),
        TestCoin::cw20("USDD"),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64).unwrap();

    let user1 = Addr::unchecked("user1");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000),
        helper.assets[&test_coins[1]].with_balance(100_000000),
        helper.assets[&test_coins[2]].with_balance(100_000000),
    ];
    helper.give_me_money(&assets, &user1);

    helper.provide_liquidity(&user1, &assets).unwrap();

    assert_eq!(300_000000, helper.token_balance(&helper.lp_token, &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[2], &user1));

    // The user2 with the same assets should receive the same share
    let user2 = Addr::unchecked("user2");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000),
        helper.assets[&test_coins[1]].with_balance(100_000000),
        helper.assets[&test_coins[2]].with_balance(100_000000),
    ];
    helper.give_me_money(&assets, &user2);
    helper.provide_liquidity(&user2, &assets).unwrap();
    assert_eq!(300_000000, helper.token_balance(&helper.lp_token, &user2));

    // The user3 makes imbalanced provide thus he is charged with fees
    let user3 = Addr::unchecked("user3");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(200_000000),
        helper.assets[&test_coins[1]].with_balance(100_000000),
    ];
    helper.give_me_money(&assets, &user3);
    helper.provide_liquidity(&user3, &assets).unwrap();
    assert_eq!(299_875484, helper.token_balance(&helper.lp_token, &user3));

    // Providing last asset with explicit zero amount should give nearly the same result
    let user4 = Addr::unchecked("user4");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(200_000000),
        helper.assets[&test_coins[1]].with_balance(100_000000),
        helper.assets[&test_coins[2]].with_balance(0),
    ];
    helper.give_me_money(&assets, &user4);
    helper.provide_liquidity(&user4, &assets).unwrap();
    assert_eq!(299_682199, helper.token_balance(&helper.lp_token, &user4));

    helper
        .withdraw_liquidity(&user1, 300_000000, vec![])
        .unwrap();

    assert_eq!(0, helper.token_balance(&helper.lp_token, &user1));
    // Previous imbalanced provides resulted in different share in assets
    assert_eq!(150_055310, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(100_036873, helper.coin_balance(&test_coins[1], &user1));
    assert_eq!(50_018436, helper.coin_balance(&test_coins[2], &user1));

    // Checking imbalanced withdraw. Withdrawing only the first asset x 300 with the whole amount of LP tokens
    helper
        .withdraw_liquidity(
            &user2,
            300_000000,
            vec![helper.assets[&test_coins[0]].with_balance(300_000000)],
        )
        .unwrap();

    // Previous imbalanced provides resulted in small LP balance residual
    assert_eq!(208725, helper.token_balance(&helper.lp_token, &user2));
    assert_eq!(300_000000, helper.coin_balance(&test_coins[0], &user2));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user2));
    assert_eq!(0, helper.coin_balance(&test_coins[2], &user2));

    // Trying to receive more than possible
    let err = helper
        .withdraw_liquidity(
            &user3,
            100_000000,
            vec![helper.assets[&test_coins[1]].with_balance(101_000000)],
        )
        .unwrap_err();
    assert_eq!(
        "Generic error: Not enough LP tokens. You need 100892384 LP tokens.",
        err.root_cause().to_string()
    );

    // Providing more LP tokens than needed. The rest will be kept on the user's balance
    helper
        .withdraw_liquidity(
            &user3,
            200_892384,
            vec![helper.assets[&test_coins[1]].with_balance(101_000000)],
        )
        .unwrap();

    // initial balance - spent amount; 100 goes back to the user3
    assert_eq!(
        299_875484 - 100_892384,
        helper.token_balance(&helper.lp_token, &user3)
    );
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user3));
    assert_eq!(101_000000, helper.coin_balance(&test_coins[1], &user3));
    assert_eq!(0, helper.coin_balance(&test_coins[2], &user3));
}

#[test]
fn provide_with_different_precision() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::cw20precise("FOO", 4),
        TestCoin::cw20precise("BAR", 5),
        TestCoin::cw20precise("ADN", 6),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64).unwrap();

    for user_name in ["user1", "user2"] {
        let user = Addr::unchecked(user_name);

        let assets = vec![
            helper.assets[&test_coins[0]].with_balance(100_0000),
            helper.assets[&test_coins[1]].with_balance(100_00000),
            helper.assets[&test_coins[2]].with_balance(100_000000),
        ];
        helper.give_me_money(&assets, &user);

        helper.provide_liquidity(&user, &assets).unwrap();

        assert_eq!(300_000000, helper.token_balance(&helper.lp_token, &user));
        assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
        assert_eq!(0, helper.coin_balance(&test_coins[1], &user));
        assert_eq!(0, helper.coin_balance(&test_coins[2], &user));

        helper
            .withdraw_liquidity(&user, 300_000000, vec![])
            .unwrap();

        assert_eq!(0, helper.token_balance(&helper.lp_token, &user));
        assert_eq!(100_0000, helper.coin_balance(&test_coins[0], &user));
        assert_eq!(100_00000, helper.coin_balance(&test_coins[1], &user));
        assert_eq!(100_000000, helper.coin_balance(&test_coins[2], &user));
    }
}

#[test]
fn swap_different_precisions() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::cw20precise("FOO", 4),
        TestCoin::cw20precise("BAR", 5),
        TestCoin::cw20precise("ADN", 6),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_0000),
        helper.assets[&test_coins[1]].with_balance(100_000_00000),
        helper.assets[&test_coins[2]].with_balance(100_000_000000),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let user = Addr::unchecked("user");
    // 100 x FOO tokens
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_0000);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper
        .swap(
            &user,
            &offer_asset,
            Some(helper.assets[&test_coins[2]].clone()),
        )
        .unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
    // 99.999010 x ADN tokens
    assert_eq!(99_999010, helper.coin_balance(&test_coins[2], &user));
}

#[test]
fn check_swaps() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::native("uluna"),
        TestCoin::cw20("USDC"),
        TestCoin::cw20("USDD"),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000),
        helper.assets[&test_coins[1]].with_balance(100_000_000000),
        helper.assets[&test_coins[2]].with_balance(100_000_000000),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000);
    helper.give_me_money(&[offer_asset.clone()], &user);

    let err = helper.swap(&user, &offer_asset, None).unwrap_err();
    assert_eq!(
        ContractError::VariableAssetMissed {},
        err.downcast().unwrap()
    );

    let err = helper
        .swap(
            &user,
            &offer_asset,
            Some(helper.assets[&test_coins[0]].clone()),
        )
        .unwrap_err();
    assert_eq!(ContractError::SameAssets {}, err.downcast().unwrap());

    helper
        .swap(
            &user,
            &offer_asset,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
    assert_eq!(99_999010, helper.coin_balance(&test_coins[1], &user));
}

#[test]
fn check_wrong_initializations() {
    let owner = Addr::unchecked("owner");

    let err = Helper::new(&owner, vec![TestCoin::native("uluna")], 100u64).unwrap_err();

    assert_eq!(
        ContractError::InvalidNumberOfAssets {},
        err.downcast().unwrap()
    );

    let err = Helper::new(
        &owner,
        vec![
            TestCoin::native("one"),
            TestCoin::cw20("two"),
            TestCoin::native("three"),
            TestCoin::cw20("four"),
            TestCoin::native("five"),
            TestCoin::cw20("six"),
        ],
        100u64,
    )
    .unwrap_err();

    assert_eq!(
        ContractError::InvalidNumberOfAssets {},
        err.downcast().unwrap()
    );

    let err = Helper::new(
        &owner,
        vec![
            TestCoin::native("uluna"),
            TestCoin::native("uluna"),
            TestCoin::cw20("USDC"),
        ],
        100u64,
    )
    .unwrap_err();

    assert_eq!(ContractError::DoublingAssets {}, err.downcast().unwrap());

    // 5 assets in the pool is okay
    Helper::new(
        &owner,
        vec![
            TestCoin::native("one"),
            TestCoin::cw20("two"),
            TestCoin::native("three"),
            TestCoin::cw20("four"),
            TestCoin::native("five"),
        ],
        100u64,
    )
    .unwrap();
}
