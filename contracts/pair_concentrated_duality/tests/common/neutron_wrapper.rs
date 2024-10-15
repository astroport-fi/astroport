#![cfg(not(tarpaulin_include))]
#![cfg(feature = "test-tube")]
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;
use std::{process::Command, str::FromStr};

use anyhow::Result as AnyResult;
use cosmwasm_schema::serde::de::{DeserializeOwned, Error};
use cosmwasm_schema::serde::Serialize;
use cosmwasm_std::{coin, Coin, Decimal256, Event, Fraction, Uint128};
use neutron_test_tube::{
    Account, Bank, Dex, ExecuteResponse, Module, NeutronTestApp, SigningAccount, Wasm,
};

const BUILD_CONTRACTS: &[&str] = &[
    "astroport-pair-concentrated-duality",
    "astroport-pair-concentrated",
    "astroport-factory",
    "astroport-native-coin-registry",
];

fn locate_workspace_root() -> String {
    let result = Command::new("cargo")
        .args(&["locate-project", "--workspace", "--message-format=plain"])
        .output()
        .expect("failed to locate workspace root");

    String::from_utf8(result.stdout)
        .unwrap()
        .trim_end()
        .strip_suffix("Cargo.toml")
        .unwrap()
        .to_string()
}

pub struct TestAppWrapper<'a> {
    pub signer: SigningAccount,
    pub app: &'a NeutronTestApp,
    pub wasm: Wasm<'a, NeutronTestApp>,
    pub bank: Bank<'a, NeutronTestApp>,
    pub dex: Dex<'a, NeutronTestApp>,
    pub code_ids: HashMap<&'a str, u64>,
}

impl<'a> TestAppWrapper<'a> {
    pub fn bootstrap(app: &'a NeutronTestApp) -> AnyResult<Self> {
        let project_dir = locate_workspace_root();

        // Build contracts
        for contract in BUILD_CONTRACTS {
            let output = Command::new("cargo")
                .args(&[
                    "build",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--release",
                    "--lib",
                    "--locked",
                    "--package",
                    contract,
                ])
                .current_dir(&project_dir)
                .output()
                .expect(&format!("failed to build contract {}", contract));
            assert!(
                output.status.success(),
                "failed to build contracts: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let target_dir = Path::new(&project_dir).join("target/wasm32-unknown-unknown/release");

        let native_registry_wasm = target_dir.join("astroport_native_coin_registry.wasm");
        let factory_wasm = target_dir.join("astroport_factory.wasm");
        let cl_pool_wasm = target_dir.join("astroport_pair_concentrated.wasm");
        let cl_pool_duality_wasm = target_dir.join("astroport_pair_concentrated_duality.wasm");

        let signer = app
            .init_account(&[coin(10000e18 as u128, "untrn")])
            .unwrap();

        let mut helper = Self {
            signer,
            app,
            wasm: Wasm::new(app),
            dex: Dex::new(app),
            bank: Bank::new(&app),
            code_ids: HashMap::new(),
        };

        // Store Astroport contracts

        println!("Storing cl pool contract...");
        let cl_pair_code_id = helper.store_code(&cl_pool_wasm)?;
        helper.code_ids.insert("pair-concentrated", cl_pair_code_id);

        println!("Storing cl pool duality contract...");
        let cl_pair_inj_code_id = helper.store_code(&cl_pool_duality_wasm)?;
        helper
            .code_ids
            .insert("pair-concentrated-duality", cl_pair_inj_code_id);

        println!("Storing coin registry contract...");
        let native_registry_code_id = helper.store_code(&native_registry_wasm)?;
        helper
            .code_ids
            .insert("coin-registry", native_registry_code_id);

        println!("Storing factory contract...");
        let factory_code_id = helper.store_code(&factory_wasm)?;
        helper.code_ids.insert("factory", factory_code_id);

        Ok(helper)
    }

    pub fn store_code<P>(&self, contract_path: P) -> AnyResult<u64>
    where
        P: AsRef<Path>,
    {
        // Load the contract wasm bytecode
        let wasm_byte_code = std::fs::read(contract_path)?;

        // Store the code
        self.wasm
            .store_code(&wasm_byte_code, None, &self.signer)
            .map(|res| res.data.code_id)
            .map_err(Into::into)
    }

    pub fn store_and_init<P, T>(&self, contract_path: P, instantiate_msg: &T) -> AnyResult<String>
    where
        T: ?Sized + Serialize,
        P: AsRef<Path>,
    {
        let code_id = self.store_code(contract_path)?;

        // Instantiate the contract
        self.init_contract(code_id, instantiate_msg, &[])
    }

    pub fn init_contract<T>(&self, code_id: u64, msg: &T, funds: &[Coin]) -> AnyResult<String>
    where
        T: ?Sized + Serialize,
    {
        self.wasm
            .instantiate(
                code_id,
                msg,
                Some(&self.signer.address()),
                Some("Test label"),
                funds,
                &self.signer,
            )
            .map(|res| res.data.address.to_string())
            .map_err(|e| e.into())
    }

    pub fn execute_contract(
        &self,
        sender: &SigningAccount,
        contract_addr: &str,
        msg: &impl Serialize,
        funds: &[Coin],
    ) -> AnyResult<
        ExecuteResponse<
            neutron_test_tube::cosmrs::proto::cosmwasm::wasm::v1::MsgExecuteContractResponse,
        >,
    > {
        self.wasm
            .execute(contract_addr, msg, funds, sender)
            .map_err(Into::into)
    }

    pub fn smart_query<T, R>(&self, contract: &str, query: &T) -> AnyResult<R>
    where
        T: ?Sized + Serialize,
        R: ?Sized + DeserializeOwned,
    {
        self.wasm.query(contract, query).map_err(Into::into)
    }

    pub fn next_block(&self) -> () {
        self.app.increase_time(5)
    }
}

fn find_attribute(events: &[Event], key: &str) -> Option<String> {
    for event in events {
        for attr in &event.attributes {
            if attr.key == key {
                return Some(attr.value.to_string());
            }
        }
    }

    None
}

pub fn f64_to_dec<T>(val: f64) -> T
where
    T: FromStr,
    T::Err: Error,
{
    T::from_str(&val.to_string()).unwrap()
}

pub struct SdkDec<T = Decimal256> {
    pub value: T,
}

impl<T> SdkDec<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl Into<String> for SdkDec {
    fn into(self) -> String {
        self.value.atomics().to_string()
    }
}

impl Into<u128> for SdkDec {
    fn into(self) -> u128 {
        let uint128: Uint128 = self.value.numerator().try_into().unwrap();
        uint128.u128()
    }
}

impl From<f64> for SdkDec {
    fn from(val: f64) -> Self {
        Self::new(Decimal256::from_str(&val.to_string()).unwrap())
    }
}
