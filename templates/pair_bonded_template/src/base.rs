use crate::error::ContractError;
use crate::state::CONFIG;
use astroport::asset::{addr_opt_validate, addr_validate_to_lower, Asset, AssetInfo, PairInfo};
use astroport::factory::PairType;
use astroport::pair::{
    migration_check, ConfigResponse, CumulativePricesResponse, Cw20HookMsg, InstantiateMsg,
    PoolResponse, ReverseSimulationResponse, SimulationResponse,
};
use astroport::pair_bonded::{
    Config, ExecuteMsg, QueryMsg, DEFAULT_SLIPPAGE, MAX_ALLOWED_SLIPPAGE,
};
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;
use std::str::FromStr;

pub trait PairBonded<'a> {
    /// Contract name that is used for migration.
    const CONTRACT_NAME: &'a str = "astroport-pair-bonded";
    /// Contract version that is used for migration.
    const CONTRACT_VERSION: &'a str = env!("CARGO_PKG_VERSION");

    /// ## Description
    /// Creates a new contract with the specified parameters in [`InstantiateMsg`].
    /// Returns a [`Response`] with the specified attributes if the operation was successful,
    /// or a [`ContractError`] if the contract was not created.
    /// ## Params
    /// * **deps** is an object of type [`DepsMut`].
    ///
    /// * **env** is an object of type [`Env`].
    ///
    /// * **_info** is an object of type [`MessageInfo`].
    ///
    /// * **msg** is a message of type [`InstantiateMsg`] which contains the parameters for creating the contract.
    fn instantiate(
        &self,
        deps: DepsMut,
        env: Env,
        _info: MessageInfo,
        msg: InstantiateMsg,
    ) -> Result<Response, ContractError> {
        msg.asset_infos[0].check(deps.api)?;
        msg.asset_infos[1].check(deps.api)?;

        if msg.asset_infos[0] == msg.asset_infos[1] {
            return Err(ContractError::DoublingAssets {});
        }

        set_contract_version(deps.storage, Self::CONTRACT_NAME, Self::CONTRACT_VERSION)?;

        let config = Config {
            pair_info: PairInfo {
                contract_addr: env.contract.address,
                liquidity_token: Addr::unchecked(""),
                asset_infos: msg.asset_infos.clone(),
                pair_type: PairType::Custom(String::from("Bonded")),
            },
            factory_addr: addr_validate_to_lower(deps.api, msg.factory_addr)?,
        };

        CONFIG.save(deps.storage, &config)?;

        Ok(Response::new())
    }

    /// ## Description
    /// Exposes all the execute functions available in the contract.
    /// ## Params
    /// * **deps** is an object of type [`Deps`].
    ///
    /// * **env** is an object of type [`Env`].
    ///
    /// * **info** is an object of type [`MessageInfo`].
    ///
    /// * **msg** is an object of type [`ExecuteMsg`].
    ///
    /// ## Queries
    /// * **ExecuteMsg::UpdateConfig { params: Binary }**  Not supported.
    ///
    /// * **ExecuteMsg::Receive(msg)** Receives a message of type [`Cw20ReceiveMsg`] and processes
    /// it depending on the received template.
    ///
    /// * **ExecuteMsg::ProvideLiquidity {
    ///             assets,
    ///             slippage_tolerance,
    ///             auto_stake,
    ///             receiver,
    ///         }**  Not supported.
    ///
    /// * **ExecuteMsg::Swap {
    ///             offer_asset,
    ///             belief_price,
    ///             max_spread,
    ///             to,
    ///         }** Performs an swap using the specified parameters. (It needs to be implemented)
    ///
    /// * **ExecuteMsg::AssertAndSend {
    ///             offer_asset,
    ///             belief_price,
    ///             max_spread,
    ///             ask_asset_info,
    ///             receiver,
    ///             sender,
    ///         }** (internal) Is used as a sub-execution to send received tokens to the receiver and check the spread/price.
    fn execute(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: ExecuteMsg,
    ) -> Result<Response, ContractError> {
        let cfg = CONFIG.load(deps.storage)?;

        if migration_check(deps.querier, &cfg.factory_addr, &env.contract.address)? {
            return Err(ContractError::PairIsNotMigrated {});
        }

        match msg {
            ExecuteMsg::UpdateConfig { .. } => Err(ContractError::NotSupported {}),
            ExecuteMsg::Receive(msg) => self.receive_cw20(deps, env, info, msg),
            ExecuteMsg::ProvideLiquidity { .. } => Err(ContractError::NotSupported {}),
            ExecuteMsg::Swap {
                offer_asset,
                belief_price,
                max_spread,
                to,
            } => self.execute_swap(deps, env, info, offer_asset, belief_price, max_spread, to),
            ExecuteMsg::AssertAndSend {
                offer_asset,
                belief_price,
                max_spread,
                ask_asset_info,
                receiver,
                sender,
            } => self.assert_receive_and_send(
                deps,
                env,
                info,
                sender,
                offer_asset,
                ask_asset_info,
                receiver,
                belief_price,
                max_spread,
            ),
        }
    }

    /// ## Description
    /// Exposes all the queries available in the contract.
    /// ## Params
    /// * **deps** is an object of type [`Deps`].
    ///
    /// * **_env** is an object of type [`Env`].
    ///
    /// * **msg** is an object of type [`QueryMsg`].
    ///
    /// ## Queries
    /// * **QueryMsg::Pair {}** Returns information about the pair in an object of type [`PairInfo`].
    ///
    /// * **QueryMsg::Pool {}** Returns information about the amount of assets in the pair contract as
    /// well as the amount of LP tokens issued using an object of type [`PoolResponse`].
    ///
    /// * **QueryMsg::Share { amount }** Returns the amount of assets that could be withdrawn from the pool
    /// using a specific amount of LP tokens. The result is returned in a vector that contains objects of type [`Asset`].
    ///
    /// * **QueryMsg::Simulation { offer_asset }** Returns the result of a swap simulation using a [`SimulationResponse`] object.
    ///
    /// * **QueryMsg::ReverseSimulation { ask_asset }** Returns the result of a reverse swap simulation using
    /// a [`ReverseSimulationResponse`] object.
    ///
    /// * **QueryMsg::CumulativePrices {}** Returns information about cumulative prices for the assets in the
    /// pool using a [`CumulativePricesResponse`] object.
    ///
    /// * **QueryMsg::Config {}** Returns the configuration for the pair contract using a [`ConfigResponse`] object.
    fn query(&self, deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
        match msg {
            QueryMsg::Pair {} => to_binary(&self.query_pair_info(deps)?),
            QueryMsg::Pool {} => to_binary(&self.query_pool(deps)?),
            QueryMsg::Share { .. } => to_binary(&Vec::<Asset>::new()),
            QueryMsg::Simulation { offer_asset } => {
                to_binary(&self.query_simulation(deps, env, offer_asset)?)
            }
            QueryMsg::ReverseSimulation { ask_asset } => {
                to_binary(&self.query_reverse_simulation(deps, env, ask_asset)?)
            }
            QueryMsg::CumulativePrices {} => to_binary(&self.query_cumulative_prices(deps, env)?),
            QueryMsg::Config {} => to_binary(&self.query_config(deps)?),
        }
    }

    /// ## Description
    /// Receives a message of type [`Cw20ReceiveMsg`] and processes it depending on the received template.
    /// If no template is not found in the received message, then an [`ContractError`] is returned,
    /// otherwise it returns a [`Response`] with the specified attributes if the operation was successful
    /// ## Params
    /// * **deps** is an object of type [`DepsMut`].
    ///
    /// * **env** is an object of type [`Env`].
    ///
    /// * **info** is an object of type [`MessageInfo`].
    ///
    /// * **cw20_msg** is an object of type [`Cw20ReceiveMsg`]. This is the CW20 receive message to process.
    fn receive_cw20(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        cw20_msg: Cw20ReceiveMsg,
    ) -> Result<Response, ContractError> {
        match from_binary(&cw20_msg.msg) {
            Ok(Cw20HookMsg::Swap {
                belief_price,
                max_spread,
                to,
            }) => {
                // Only asset contract can execute this message
                let mut authorized = false;
                let config = CONFIG.load(deps.storage)?;

                for pool in config.pair_info.asset_infos {
                    if let AssetInfo::Token { contract_addr, .. } = &pool {
                        if contract_addr == &info.sender {
                            authorized = true;
                        }
                    }
                }

                if !authorized {
                    return Err(ContractError::Unauthorized {});
                }

                let to_addr = addr_opt_validate(deps.api, &to)?;
                let contract_addr = info.sender.clone();
                let sender = addr_validate_to_lower(deps.api, cw20_msg.sender)?;
                self.swap(
                    deps,
                    env,
                    info,
                    sender,
                    Asset {
                        info: AssetInfo::Token { contract_addr },
                        amount: cw20_msg.amount,
                    },
                    belief_price,
                    max_spread,
                    to_addr,
                )
            }
            Ok(Cw20HookMsg::WithdrawLiquidity {}) => Err(ContractError::NotSupported {}),
            Err(err) => Err(err.into()),
        }
    }

    /// ## Description
    /// Performs an swap operation with the specified parameters. The trader must approve the
    /// pool contract to transfer offer assets from their wallet.
    /// Returns an [`ContractError`] on failure, otherwise returns the [`Response`] with the specified attributes if the operation was successful.
    /// ## Params
    /// * **deps** is an object of type [`DepsMut`].
    ///
    /// * **env** is an object of type [`Env`].
    ///
    /// * **info** is an object of type [`MessageInfo`].
    ///
    /// * **sender** is an object of type [`Addr`]. This is the sender of the swap operation.
    ///
    /// * **offer_asset** is an object of type [`Asset`]. Proposed asset for swapping.
    ///
    /// * **belief_price** is an object of type [`Option<Decimal>`]. Used to calculate the maximum swap spread.
    ///
    /// * **max_spread** is an object of type [`Option<Decimal>`]. Sets the maximum spread of the swap operation.
    ///
    /// * **to** is an object of type [`Option<Addr>`]. Sets the recipient of the swap operation.
    /// NOTE - the address that wants to swap should approve the pair contract to pull the offer token.
    #[allow(clippy::too_many_arguments)]
    fn execute_swap(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        offer_asset: Asset,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<String>,
    ) -> Result<Response, ContractError> {
        offer_asset.info.check(deps.api)?;
        if !offer_asset.is_native_token() {
            return Err(ContractError::Unauthorized {});
        }

        let to_addr = addr_opt_validate(deps.api, &to)?;

        self.swap(
            deps,
            env,
            info.clone(),
            info.sender,
            offer_asset,
            belief_price,
            max_spread,
            to_addr,
        )
    }

    /// ## Description
    /// Performs a swap with the specified parameters.
    /// ### Should be implemented
    #[allow(clippy::too_many_arguments)]
    fn swap(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        sender: Addr,
        offer_asset: Asset,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<Addr>,
    ) -> Result<Response, ContractError>;

    /// ## Description
    /// Returns information about the pair contract in an object of type [`PairInfo`].
    /// ## Params
    /// * **deps** is an object of type [`Deps`].
    fn query_pair_info(&self, deps: Deps) -> StdResult<PairInfo> {
        let config = CONFIG.load(deps.storage)?;
        Ok(config.pair_info)
    }

    /// ## Description
    /// Returns the amounts of assets in the pair contract in an object of type [`PoolResponse`].
    /// ## Params
    /// * **deps** is an object of type [`Deps`].
    fn query_pool(&self, deps: Deps) -> StdResult<PoolResponse> {
        let config = CONFIG.load(deps.storage)?;
        let (assets, total_share) = self.pool_info(&config)?;

        let resp = PoolResponse {
            assets,
            total_share,
        };

        Ok(resp)
    }

    /// ## Description
    /// Returns information about a swap simulation in a [`SimulationResponse`] object.
    /// ### Should be implemented
    fn query_simulation(
        &self,
        deps: Deps,
        env: Env,
        offer_asset: Asset,
    ) -> StdResult<SimulationResponse>;

    /// ## Description
    /// Returns information about a reverse swap simulation in a [`ReverseSimulationResponse`] object.
    /// ### Should be implemented
    fn query_reverse_simulation(
        &self,
        deps: Deps,
        env: Env,
        ask_asset: Asset,
    ) -> StdResult<ReverseSimulationResponse>;

    /// ## Description
    /// Returns information about cumulative prices for the assets in the pool using a [`CumulativePricesResponse`] object.
    /// ## Params
    /// * **deps** is an object of type [`Deps`].
    ///
    /// * **env** is an object of type [`Env`].
    fn query_cumulative_prices(
        &self,
        deps: Deps,
        _env: Env,
    ) -> StdResult<CumulativePricesResponse> {
        let config = CONFIG.load(deps.storage)?;
        let (assets, total_share) = self.pool_info(&config)?;

        let resp = CumulativePricesResponse {
            assets,
            total_share,
            price0_cumulative_last: Uint128::zero(),
            price1_cumulative_last: Uint128::zero(),
        };

        Ok(resp)
    }

    /// ## Description
    /// Returns the pair contract configuration in a [`ConfigResponse`] object.
    /// ## Params
    /// * **deps** is an object of type [`Deps`].
    fn query_config(&self, _deps: Deps) -> StdResult<ConfigResponse> {
        Ok(ConfigResponse {
            block_time_last: 0u64,
            params: None,
        })
    }

    /// ## Description
    /// Returns the total amount of assets in the pool.
    /// ## Params
    /// * **config** is an object of type [`Config`].
    fn pool_info(&self, config: &Config) -> StdResult<([Asset; 2], Uint128)> {
        let pools: [Asset; 2] = [
            Asset {
                amount: Uint128::zero(),
                info: config.pair_info.asset_infos[0].clone(),
            },
            Asset {
                amount: Uint128::zero(),
                info: config.pair_info.asset_infos[1].clone(),
            },
        ];

        Ok((pools, Uint128::zero()))
    }

    /// ## Description
    /// Performs an swap operation with the specified parameters. The trader must approve the
    /// pool contract to transfer offer assets from their wallet.
    /// Returns an [`ContractError`] on failure, otherwise returns the [`Response`] with the specified attributes if the operation was successful.
    /// ## Params
    /// * **deps** is an object of type [`DepsMut`].
    ///
    /// * **env** is an object of type [`Env`].
    ///
    /// * **info** is an object of type [`MessageInfo`].
    ///
    /// * **sender** is an object of type [`Addr`]. This is the sender of the swap operation.
    ///
    /// * **offer_asset** is an object of type [`Asset`]. Proposed asset for swapping.
    ///
    /// * **ask_asset_info** is an object of type [`Addr`]. Ask asset info.
    ///
    /// * **receiver** is an object of type [`Addr`]. This is the receiver of the swap operation.
    ///
    /// * **belief_price** is an object of type [`Option<Decimal>`]. Used to calculate the maximum swap spread.
    ///
    /// * **max_spread** is an object of type [`Option<Decimal>`]. Sets the maximum spread of the swap operation.
    #[allow(clippy::too_many_arguments)]
    fn assert_receive_and_send(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        sender: Addr,
        offer_asset: Asset,
        ask_asset_info: AssetInfo,
        receiver: Addr,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
    ) -> Result<Response, ContractError> {
        if env.contract.address != info.sender {
            // Only allowed to be sent by the contract itself
            return Err(ContractError::Unauthorized {});
        }

        let offer_amount = offer_asset.amount;
        let return_amount = ask_asset_info.query_pool(&deps.querier, env.contract.address)?;

        // Check the max spread limit (if it was specified)
        self.assert_max_spread(belief_price, max_spread, offer_amount, return_amount)?;

        // Compute the tax for the receiving asset (if it is a native one)
        let return_asset = Asset {
            info: ask_asset_info.clone(),
            amount: return_amount,
        };

        let tax_amount = return_asset.compute_tax(&deps.querier)?;

        Ok(Response::new()
            .add_message(return_asset.into_msg(&deps.querier, receiver.clone())?)
            .add_attribute("action", "swap")
            .add_attribute("sender", sender.to_string())
            .add_attribute("receiver", receiver.to_string())
            .add_attribute("offer_asset", offer_asset.info.to_string())
            .add_attribute("ask_asset", ask_asset_info.to_string())
            .add_attribute("offer_amount", offer_amount.to_string())
            .add_attribute("return_amount", return_amount.to_string())
            .add_attribute("tax_amount", tax_amount.to_string())
            .add_attribute("spread_amount", "0")
            .add_attribute("commission_amount", "0")
            .add_attribute("maker_fee_amount", "0"))
    }

    /// ## Description
    /// Returns a [`ContractError`] on failure.
    /// If `belief_price` and `max_spread` are both specified, we compute a new spread,
    /// otherwise we just use the swap spread to check `max_spread`.
    /// ## Params
    /// * **belief_price** is an object of type [`Option<Decimal>`]. This is the belief price used in the swap.
    ///
    /// * **max_spread** is an object of type [`Option<Decimal>`]. This is the
    /// max spread allowed so that the swap can be executed successfuly.
    ///
    /// * **offer_amount** is an object of type [`Uint128`]. This is the amount of assets to swap.
    ///
    /// * **return_amount** is an object of type [`Uint128`]. This is the amount of assets to receive from the swap.
    fn assert_max_spread(
        &self,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        offer_amount: Uint128,
        return_amount: Uint128,
    ) -> Result<(), ContractError> {
        let default_spread = Decimal::from_str(DEFAULT_SLIPPAGE)?;
        let max_allowed_spread = Decimal::from_str(MAX_ALLOWED_SLIPPAGE)?;

        let max_spread = max_spread.unwrap_or(default_spread);
        if max_spread.gt(&max_allowed_spread) {
            return Err(ContractError::AllowedSpreadAssertion {});
        }

        if let Some(belief_price) = belief_price {
            let expected_return = offer_amount * (Decimal::one() / belief_price);
            let spread_amount = expected_return.saturating_sub(return_amount);

            if return_amount < expected_return
                && Decimal::from_ratio(spread_amount, expected_return) > max_spread
            {
                return Err(ContractError::MaxSpreadAssertion {});
            }
        }

        Ok(())
    }
}
