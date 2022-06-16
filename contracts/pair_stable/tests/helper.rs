use std::collections::HashMap;

use anyhow::Result as AnyResult;
use cosmwasm_std::{coin, to_binary, Addr, Coin, Empty, Uint128};
use cw20::{BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20QueryMsg};
use cw_multi_test::{App, AppResponse, Contract, ContractWrapper, Executor};
use derivative::Derivative;
use itertools::Itertools;

use astroport::asset::{native_asset_info, token_asset_info, Asset, AssetInfo, PairInfo};
use astroport::pair::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StablePoolParams};
use astroport::querier::NATIVE_TOKEN_PRECISION;
use astroport_pair_stable::contract::{execute, instantiate, query, reply};

const INIT_BALANCE: u128 = 1_000_000_000000;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TestCoin {
    Cw20(String),
    Cw20Precise(String, u8),
    Native(String),
}

impl TestCoin {
    pub fn denom(&self) -> Option<String> {
        match self {
            TestCoin::Native(denom) => Some(denom.clone()),
            _ => None,
        }
    }

    pub fn cw20_init_data(&self) -> Option<(String, u8)> {
        match self {
            TestCoin::Cw20(name) => Some((name.clone(), 6u8)),
            TestCoin::Cw20Precise(name, precision) => Some((name.clone(), *precision)),
            _ => None,
        }
    }

    pub fn native(denom: &str) -> Self {
        Self::Native(denom.to_string())
    }

    pub fn cw20(name: &str) -> Self {
        Self::Cw20(name.to_string())
    }

    pub fn cw20precise(name: &str, precision: u8) -> Self {
        Self::Cw20Precise(name.to_string(), precision)
    }
}

pub fn init_native_coins(test_coins: &[TestCoin]) -> Vec<Coin> {
    test_coins
        .iter()
        .filter_map(|test_coin| match test_coin {
            TestCoin::Native(name) => {
                let init_balance = INIT_BALANCE * 10u128.pow(NATIVE_TOKEN_PRECISION as u32);
                Some(coin(init_balance, name))
            }
            _ => None,
        })
        .collect()
}

fn token_contract() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new_with_empty(
        astroport_token::contract::execute,
        astroport_token::contract::instantiate,
        astroport_token::contract::query,
    ))
}

fn pair_contract() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new_with_empty(execute, instantiate, query).with_reply_empty(reply))
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Helper {
    #[derivative(Debug = "ignore")]
    pub app: App,
    pub owner: Addr,
    pub assets: HashMap<TestCoin, AssetInfo>,
    pub pair_addr: Addr,
    pub lp_token: Addr,
    pub amp: u64,
}

impl Helper {
    pub fn new(owner: &Addr, test_coins: Vec<TestCoin>, amp: u64) -> AnyResult<Self> {
        let mut app = App::new(|router, _, storage| {
            router
                .bank
                .init_balance(storage, &owner, init_native_coins(&test_coins))
                .unwrap()
        });

        let mut asset_infos_vec: Vec<_> = test_coins
            .clone()
            .into_iter()
            .filter_map(|coin| Some((coin.clone(), native_asset_info(coin.denom()?))))
            .collect();

        let token_code_id = app.store_code(token_contract());

        test_coins.clone().into_iter().for_each(|coin| {
            if let Some((name, decimals)) = coin.cw20_init_data() {
                let token_addr = Self::init_token(&mut app, token_code_id, name, decimals, &owner);
                asset_infos_vec.push((coin, token_asset_info(token_addr)))
            }
        });

        let init_msg = InstantiateMsg {
            asset_infos: asset_infos_vec
                .clone()
                .into_iter()
                .map(|(_, asset_info)| asset_info)
                .collect(),
            token_code_id,
            factory_addr: "factory".to_string(),
            init_params: Some(to_binary(&StablePoolParams { amp }).unwrap()),
        };

        let pair_code_id = app.store_code(pair_contract());

        let pair_addr = app.instantiate_contract(
            pair_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "pair_label",
            None,
        )?;

        let resp: PairInfo = app
            .wrap()
            .query_wasm_smart(&pair_addr, &QueryMsg::Pair {})
            .unwrap();

        Ok(Self {
            app,
            owner: owner.clone(),
            assets: asset_infos_vec.into_iter().collect(),
            pair_addr,
            lp_token: resp.liquidity_token,
            amp,
        })
    }

    pub fn provide_liquidity(&mut self, sender: &Addr, assets: &[Asset]) -> AnyResult<AppResponse> {
        let funds =
            assets.mock_coins_sent(&mut self.app, sender, &self.pair_addr, SendType::Allowance);

        let msg = ExecuteMsg::ProvideLiquidity {
            assets: assets.clone().to_vec(),
            slippage_tolerance: None,
            auto_stake: None,
            receiver: None,
        };

        self.app
            .execute_contract(sender.clone(), self.pair_addr.clone(), &msg, &funds)
    }

    pub fn withdraw_liquidity(
        &mut self,
        sender: &Addr,
        amount: u128,
        assets: Vec<Asset>,
    ) -> AnyResult<AppResponse> {
        let msg = Cw20ExecuteMsg::Send {
            contract: self.pair_addr.to_string(),
            amount: Uint128::from(amount),
            msg: to_binary(&Cw20HookMsg::WithdrawLiquidity { assets }).unwrap(),
        };

        self.app
            .execute_contract(sender.clone(), self.lp_token.clone(), &msg, &[])
    }

    fn init_token(
        app: &mut App,
        token_code: u64,
        name: String,
        decimals: u8,
        owner: &Addr,
    ) -> Addr {
        let init_balance = INIT_BALANCE * 10u128.pow(decimals as u32);
        app.instantiate_contract(
            token_code,
            owner.clone(),
            &astroport::token::InstantiateMsg {
                symbol: name.to_string(),
                name,
                decimals,
                initial_balances: vec![Cw20Coin {
                    address: owner.to_string(),
                    amount: Uint128::from(init_balance),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "{name}_token",
            None,
        )
        .unwrap()
    }

    pub fn token_balance(&self, token_addr: &Addr, user: &Addr) -> u128 {
        let resp: BalanceResponse = self
            .app
            .wrap()
            .query_wasm_smart(
                token_addr,
                &Cw20QueryMsg::Balance {
                    address: user.to_string(),
                },
            )
            .unwrap();

        resp.balance.u128()
    }

    pub fn coin_balance(&self, coin: &TestCoin, user: &Addr) -> u128 {
        match &self.assets[coin] {
            AssetInfo::Token { contract_addr } => self.token_balance(contract_addr, user),
            AssetInfo::NativeToken { denom } => self
                .app
                .wrap()
                .query_balance(user, denom)
                .unwrap()
                .amount
                .u128(),
        }
    }

    pub fn give_me_money(&mut self, assets: &[Asset], recipient: &Addr) {
        let funds =
            assets.mock_coins_sent(&mut self.app, &self.owner, recipient, SendType::Transfer);

        self.app
            .send_tokens(self.owner.clone(), recipient.clone(), &funds)
            .unwrap();
    }
}

#[derive(Clone, Copy)]
pub enum SendType {
    Allowance,
    Transfer,
}

pub trait AssetExt {
    fn mock_coin_sent(
        &self,
        app: &mut App,
        user: &Addr,
        spender: &Addr,
        typ: SendType,
    ) -> Vec<Coin>;
}

impl AssetExt for Asset {
    fn mock_coin_sent(
        &self,
        app: &mut App,
        user: &Addr,
        spender: &Addr,
        typ: SendType,
    ) -> Vec<Coin> {
        let mut funds = vec![];
        match &self.info {
            AssetInfo::Token { contract_addr } if !self.amount.is_zero() => {
                let msg = match typ {
                    SendType::Allowance => Cw20ExecuteMsg::IncreaseAllowance {
                        spender: spender.to_string(),
                        amount: self.amount,
                        expires: None,
                    },
                    SendType::Transfer => Cw20ExecuteMsg::Transfer {
                        recipient: spender.to_string(),
                        amount: self.amount,
                    },
                };
                app.execute_contract(user.clone(), contract_addr.clone(), &msg, &[])
                    .unwrap();
            }
            AssetInfo::NativeToken { denom } if !self.amount.is_zero() => {
                funds = vec![coin(self.amount.u128(), denom)];
            }
            _ => {}
        }

        funds
    }
}

pub trait AssetsExt {
    fn mock_coins_sent(
        &self,
        app: &mut App,
        user: &Addr,
        spender: &Addr,
        typ: SendType,
    ) -> Vec<Coin>;
}

impl AssetsExt for &[Asset] {
    fn mock_coins_sent(
        &self,
        app: &mut App,
        user: &Addr,
        spender: &Addr,
        typ: SendType,
    ) -> Vec<Coin> {
        let mut funds = vec![];
        for asset in self.iter() {
            funds.extend(asset.mock_coin_sent(app, user, spender, typ));
        }
        funds
    }
}

pub trait AssetInfoExt {
    fn with_balance(&self, balance: u128) -> Asset;
}

impl AssetInfoExt for AssetInfo {
    fn with_balance(&self, balance: u128) -> Asset {
        Asset {
            info: self.clone(),
            amount: Uint128::from(balance),
        }
    }
}
