import 'dotenv/config'
import {
    newClient,
    writeArtifact,
    readArtifact,
    deployContract,
    executeContract,
    toEncodedBinary, queryContract,
} from './helpers.js'
import { join } from 'path'
import {LCDClient} from '@terra-money/terra.js';
import {sendNotification} from "./slack_notification.js";

const ARTIFACTS_PATH = '../artifacts'
const VESTING_LABEL = "Astroport Vesting"
const GENERATOR_LABEL = "Astroport Generator"

async function main() {
    const { terra, wallet } = newClient()
    console.log(`chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`)
    let network = readArtifact(terra.config.chainID)
    console.log('network:', network)

    if (!network.tokenAddress) {
        let err = new Error("Please deploy the CW20-base ASTRO token, and then set this address in the deploy config before running this script...")
        await sendNotification(err.name, err.message, err.stack)
        throw err
    }

    if (!network.multisigAddress) {
        let err = new Error("Set the proper owner multisig for the contracts")
        await sendNotification(err.name, err.message, err.stack)
        throw err
    }

    try {
        await uploadAndInitVesting(terra, wallet)
        await uploadAndInitGenerator(terra, wallet)
        // Set generator address in factory
        await updateFactoryConfig(terra, wallet)

        // setup pools
        // await registerGenerator(terra, wallet, "terra17n5sunn88hpy965mzvt3079fqx3rttnplg779g", "28303")
        // await registerGenerator(terra, wallet, "terra1m24f7k4g66gnh9f7uncp32p722v0kyt3q4l3u5", "24528")
        // await registerGenerator(terra, wallet, "terra1htw7hm40ch0hacm8qpgd24sus4h0tq3hsseatl", "47169")


        // TESTNET
        // await registerGenerator(terra, wallet, "terra1cs66g290h4x0pf6scmwm8904yc75l3m7z0lzjr", "28303")
        // await registerGenerator(terra, wallet, "terra1q8aycvr54jarwhqvlr54jr2zqttctnefw7x37q", "24528")
        // await registerGenerator(terra, wallet, "terra1jzutwpneltsys6t96vkdr2zrc6cg0ke4e6y3s0", "47169")

        // await setupVesting(terra, wallet, network)

        // Set new owner for generator
        // network = readArtifact(terra.config.chainID) // reload variables
        // console.log('Propose owner for generator. Onwership has to be claimed within 7 days')
        // await executeContract(terra, wallet, network.generatorAddress, {
        //     "propose_new_owner": {
        //         owner: network.multisigAddress,
        //         expires_in: 604800 // 7 days
        //     }
        // })

        console.log('FINISH')
    } catch (e: any) {
        let err = new Error(e.data)
        await sendNotification(err.name, err.message, err.stack)
    }

}

async function registerGenerator(terra: LCDClient, wallet: any, lp_token: string, alloc_point: string) {
    let network = readArtifact(terra.config.chainID)

     if (!network.generatorAddress) {
        console.log(`Please deploy generator contract`)
        return
    }

    await executeContract(terra, wallet, network.generatorAddress, {
        setup_pools: {
            pools: [[lp_token, alloc_point]]
        }
    })
}

async function uploadAndInitVesting(terra: LCDClient, wallet: any) {
    let network = readArtifact(terra.config.chainID)

    if (!network.vestingAddress) {
        console.log('Deploying Vesting...')
        let resp = await deployContract(
            terra,
            wallet,
            network.multisigAddress,
            join(ARTIFACTS_PATH, 'astroport_vesting.wasm'),
            {
                owner: network.multisigAddress,
                token_addr: network.tokenAddress,
            },
            VESTING_LABEL
        )
        // @ts-ignore
        network.vestingAddress = resp.shift().shift()
        console.log(`Address Vesting Contract: ${network.vestingAddress}`)
        writeArtifact(network, terra.config.chainID)
    }
}

async function uploadAndInitGenerator(terra: LCDClient, wallet: any) {
    let network = readArtifact(terra.config.chainID)

    if (!network.generatorAddress) {
        console.log('Deploying Generator...')

        let resp = await deployContract(
            terra,
            wallet,
            network.multisigAddress,
            join(ARTIFACTS_PATH, 'astroport_generator.wasm'),
            {
                owner: network.multisigAddress,
                allowed_reward_proxies: [],
                astro_token: network.tokenAddress,
                start_block: '5918639',
                tokens_per_block: String(8403094),
                vesting_contract: network.vestingAddress,
                factory: network.factoryAddress,
                whitelist_code_id: network.whitelistCodeID,
            },
            GENERATOR_LABEL
        )

        // @ts-ignore
        network.generatorAddress = resp.shift().shift()
        console.log(`Address Generator Contract: ${network.generatorAddress}`)

        writeArtifact(network, terra.config.chainID)
    }
}

async function setupVesting(terra: LCDClient, wallet: any, network: any) {
    console.log('Setting Vesting...')

    let msg = {
        register_vesting_accounts: {
            vesting_accounts: [
                {
                    address: network.generatorAddress,
                    schedules: [
                        {
                            start_point: {
                                time: 1640865600,
                                amount: String("100") // 1% on total supply at once
                            },
                            end_point: {
                                time: 1672401600,
                                amount: String("10000")
                            }
                        }
                    ]
                }
            ]
        }
    }

    console.log('Register vesting accounts:', JSON.stringify(msg))

    await executeContract(terra, wallet, network.tokenAddress, {
        "send": {
            contract: network.vestingAddress,
            amount: String("10000"),
            msg: toEncodedBinary(msg)
        }
    })
}

async function updateFactoryConfig(terra: LCDClient, wallet: any) {
    let network = readArtifact(terra.config.chainID)
    await executeContract(terra, wallet, network.factoryAddress, {
        update_config: {
            generator_address: network.generatorAddress,
        }
    })

    console.log(await queryContract(terra, network.factoryAddress, { config: {} }))
}

main().catch(console.log)
