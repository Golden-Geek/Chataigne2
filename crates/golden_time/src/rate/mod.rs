use std::num::NonZeroU32;
use std::time::Duration;

use crate::TimeError;

const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// Direction for quantization into integral ticks or nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    /// Choose the lower integer coordinate.
    Floor,
    /// Choose the higher integer coordinate, suitable for a deadline that must not be early.
    Ceil,
}

/// Positive normalized ticks per second, including rational video rates such as 60000/1001.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RationalRate {
    numerator: NonZeroU32,
    denominator: NonZeroU32,
}

impl RationalRate {
    /// Rejects zero frequency/denominator and reduces equivalent rational rates.
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, TimeError> {
        if numerator == 0 {
            return Err(TimeError::ZeroFrequency);
        }
        if denominator == 0 {
            return Err(TimeError::ZeroDenominator);
        }
        let factor = gcd(numerator, denominator);
        Ok(Self {
            numerator: NonZeroU32::new(numerator / factor).ok_or(TimeError::ZeroFrequency)?,
            denominator: NonZeroU32::new(denominator / factor).ok_or(TimeError::ZeroDenominator)?,
        })
    }

    /// Normalized tick numerator.
    pub const fn numerator(self) -> u32 {
        self.numerator.get()
    }

    /// Normalized seconds denominator.
    pub const fn denominator(self) -> u32 {
        self.denominator.get()
    }

    /// Converts elapsed time to integral ticks with explicit quantization.
    pub fn ticks_for(self, elapsed: Duration, rounding: Rounding) -> Result<u64, TimeError> {
        let numerator = elapsed
            .as_nanos()
            .checked_mul(u128::from(self.numerator()))
            .ok_or(TimeError::Overflow)?;
        let denominator = NANOS_PER_SECOND * u128::from(self.denominator());
        u64::try_from(divide_unsigned(numerator, denominator, rounding)?).map_err(|_| TimeError::Overflow)
    }

    /// Converts an absolute tick count to a duration rather than accumulating rounded periods.
    pub fn duration_for(self, ticks: u64, rounding: Rounding) -> Result<Duration, TimeError> {
        let numerator = u128::from(ticks)
            .checked_mul(NANOS_PER_SECOND)
            .and_then(|value| value.checked_mul(u128::from(self.denominator())))
            .ok_or(TimeError::Overflow)?;
        let nanos = divide_unsigned(numerator, u128::from(self.numerator()), rounding)?;
        duration_from_nanos(nanos)
    }
}

/// Normalized show-position rate. Zero pauses and negative rates run in reverse.
///
/// Applications choose transport policy; this value only describes an affine mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlaybackRate {
    numerator: i32,
    denominator: NonZeroU32,
}

impl PlaybackRate {
    /// One show nanosecond per elapsed monotonic nanosecond.
    pub const NORMAL: Self = Self {
        numerator: 1,
        denominator: NonZeroU32::MIN,
    };
    /// A constant show position while monotonic time continues.
    pub const PAUSED: Self = Self {
        numerator: 0,
        denominator: NonZeroU32::MIN,
    };

    /// Creates a reduced signed mapping rate, retaining pause/reverse intent.
    pub fn new(numerator: i32, denominator: u32) -> Result<Self, TimeError> {
        if denominator == 0 {
            return Err(TimeError::ZeroDenominator);
        }
        let factor = gcd(numerator.unsigned_abs(), denominator);
        let numerator = i32::try_from(i64::from(numerator) / i64::from(factor)).map_err(|_| TimeError::Overflow)?;
        Ok(Self {
            numerator,
            denominator: NonZeroU32::new(denominator / factor).ok_or(TimeError::ZeroDenominator)?,
        })
    }

    /// Signed normalized numerator.
    pub const fn numerator(self) -> i32 {
        self.numerator
    }

    /// Positive normalized denominator.
    pub const fn denominator(self) -> u32 {
        self.denominator.get()
    }
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

fn divide_unsigned(numerator: u128, denominator: u128, rounding: Rounding) -> Result<u128, TimeError> {
    let quotient = numerator / denominator;
    if rounding == Rounding::Ceil && !numerator.is_multiple_of(denominator) {
        quotient.checked_add(1).ok_or(TimeError::Overflow)
    } else {
        Ok(quotient)
    }
}

fn duration_from_nanos(nanos: u128) -> Result<Duration, TimeError> {
    let seconds = u64::try_from(nanos / NANOS_PER_SECOND).map_err(|_| TimeError::Overflow)?;
    let fraction = u32::try_from(nanos % NANOS_PER_SECOND).map_err(|_| TimeError::Overflow)?;
    Ok(Duration::new(seconds, fraction))
}

pub(crate) fn divide_signed(numerator: i128, denominator: i128, rounding: Rounding) -> Result<i128, TimeError> {
    if denominator == 0 {
        return Err(TimeError::ZeroDenominator);
    }
    let (numerator, denominator) = if denominator < 0 {
        (
            numerator.checked_neg().ok_or(TimeError::Overflow)?,
            denominator.checked_neg().ok_or(TimeError::Overflow)?,
        )
    } else {
        (numerator, denominator)
    };
    let quotient = numerator.div_euclid(denominator);
    if rounding == Rounding::Ceil && numerator.rem_euclid(denominator) != 0 {
        quotient.checked_add(1).ok_or(TimeError::Overflow)
    } else {
        Ok(quotient)
    }
}

#[cfg(test)]
mod tests;
