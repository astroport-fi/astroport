use cosmwasm_std::{Decimal, Decimal256};
use std::ops::RangeInclusive;

/// ## Adjustable constants
/// 0.05
pub const DEFAULT_SLIPPAGE: Decimal256 = Decimal256::raw(50000000000000000);
/// 0.5
pub const MAX_ALLOWED_SLIPPAGE: Decimal256 = Decimal256::raw(500000000000000000);

/// ## Internal constants
/// Number of coins. (2.0)
pub const N: Decimal256 = Decimal256::raw(2000000000000000000);
/// Defines fee tolerance. If k coefficient is small enough then k = 0. (0.001)
pub const FEE_TOL: Decimal256 = Decimal256::raw(1000000000000000);
/// N ^ 2
pub const N_POW2: Decimal256 = Decimal256::raw(4000000000000000000);
/// 1e-3
pub const TOL: Decimal256 = Decimal256::raw(1000000000000000);
/// halfpow tolerance (1e-10)
pub const HALFPOW_TOL: Decimal256 = Decimal256::raw(100000000);
/// 2.0
pub const TWO: Decimal256 = Decimal256::raw(2000000000000000000);
/// Iterations limit for Newton's method
pub const MAX_ITER: usize = 64;
/// TWAP constant for external oracle prices
pub const TWAP_PRECISION_DEC: Decimal256 = Decimal256::raw((1e6 * 1e18) as u128);

/// ## Validation constants
// TODO: adjust validation constants
/// 0.001
pub const MIN_FEE: Decimal = Decimal::raw(1000000000000000);
/// 0.5
pub const MAX_FEE: Decimal = Decimal::raw(500000000000000000);

/// 1e-8
pub const FEE_GAMMA_MIN: Decimal = Decimal::raw(10000000000);
/// 0.02
pub const FEE_GAMMA_MAX: Decimal = Decimal::raw(20000000000000000);

pub const REPEG_PROFIT_THRESHOLD_MIN: Decimal = Decimal::zero();
/// 0.01
pub const REPEG_PROFIT_THRESHOLD_MAX: Decimal = Decimal::raw(10000000000000000);

/// 0.00000000001
pub const PRICE_SCALE_DELTA_MIN: Decimal = Decimal::raw(10000000);
pub const PRICE_SCALE_DELTA_MAX: Decimal = Decimal::one();

pub const MA_HALF_TIME_LIMITS: RangeInclusive<u64> = 0..=(7 * 86400);

/// 0.4
pub const AMP_MIN: Decimal = Decimal::raw(4e17 as u128);
/// 400000
pub const AMP_MAX: Decimal = Decimal::raw(4e23 as u128);

/// 0.0000001
pub const GAMMA_MIN: Decimal = Decimal::raw(100000000000);
/// 0.02
pub const GAMMA_MAX: Decimal = Decimal::raw(20000000000000000);

/// The minimum time interval for updating Amplifier or Gamma
pub const MIN_AMP_CHANGING_TIME: u64 = 86400;
/// The maximum allowed change of Amplifier or Gamma (10%).
pub const MAX_CHANGE: Decimal = Decimal::raw(1e17 as u128);
