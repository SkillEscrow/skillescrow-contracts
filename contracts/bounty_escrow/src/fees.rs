use crate::errors::Error;

pub const BPS_DENOMINATOR: i128 = 10_000;

/// Protocol fee: `floor(amount * bps / 10_000)`.
///
/// `amount` must be non-negative (the contract enforces `min_amount > 0` at funding).
pub fn fee(amount: i128, bps: u32) -> Result<i128, Error> {
    amount
        .checked_mul(i128::from(bps))
        .and_then(|v| v.checked_div(BPS_DENOMINATOR))
        .ok_or(Error::Overflow)
}

/// Amount left after the fee: `amount - fee(amount, bps)`.
/// Rounding dust stays on the contributor side, so the fee rounds down.
pub fn net(amount: i128, bps: u32) -> Result<i128, Error> {
    amount.checked_sub(fee(amount, bps)?).ok_or(Error::Overflow)
}

/// The winner's share of a disputed bounty: `floor(amount * winner_bps / 10_000)`.
/// Rounding dust goes to the maintainer side, which receives `amount - portion`.
pub fn portion(amount: i128, winner_bps: u32) -> Result<i128, Error> {
    fee(amount, winner_bps)
}

#[cfg(test)]
mod test {
    use super::*;

    const BIG: i128 = i128::MAX / 10_000;

    #[test]
    fn fee_of_zero_is_zero() {
        for bps in [0, 1, 1_000, 10_000] {
            assert_eq!(fee(0, bps), Ok(0));
        }
    }

    #[test]
    fn fee_rounds_down() {
        assert_eq!(fee(1, 1_000), Ok(0));
        assert_eq!(fee(9, 1_000), Ok(0));
        assert_eq!(fee(10, 1_000), Ok(1));
        assert_eq!(fee(19, 1_000), Ok(1));
    }

    #[test]
    fn fee_bps_boundaries() {
        assert_eq!(fee(10_000, 0), Ok(0));
        assert_eq!(fee(10_000, 1), Ok(1));
        assert_eq!(fee(10_000, 1_000), Ok(1_000));
        assert_eq!(fee(10_000, 10_000), Ok(10_000));
        assert_eq!(fee(1, 10_000), Ok(1));
    }

    #[test]
    fn fee_at_largest_safe_amount() {
        assert_eq!(fee(BIG, 0), Ok(0));
        assert_eq!(fee(BIG, 10_000), Ok(BIG));
        assert_eq!(fee(BIG, 1_000), Ok(BIG / 10));
        assert_eq!(fee(BIG, 1), Ok(BIG / 10_000));
    }

    #[test]
    fn fee_overflows_instead_of_wrapping() {
        assert_eq!(fee(i128::MAX, 10_000), Err(Error::Overflow));
        assert_eq!(fee(i128::MAX, 2), Err(Error::Overflow));
        // bps 1 is the largest multiplier that still fits.
        assert_eq!(fee(i128::MAX, 1), Ok(i128::MAX / 10_000));
        assert_eq!(fee(BIG + 1, 10_000), Err(Error::Overflow));
        // bps 0 never overflows.
        assert_eq!(fee(i128::MAX, 0), Ok(0));
    }

    #[test]
    fn net_gives_dust_to_contributor() {
        assert_eq!(net(1, 1_000), Ok(1));
        assert_eq!(net(9, 1_000), Ok(9));
        assert_eq!(net(10, 1_000), Ok(9));
        assert_eq!(net(100, 0), Ok(100));
        assert_eq!(net(100, 10_000), Ok(0));
        assert_eq!(net(0, 1_000), Ok(0));
    }

    #[test]
    fn net_plus_fee_equals_amount() {
        for amount in [0, 1, 7, 99, 100, 12_345_678, BIG] {
            for bps in [0, 1, 300, 1_000, 10_000] {
                let f = fee(amount, bps).unwrap();
                let n = net(amount, bps).unwrap();
                assert_eq!(f + n, amount, "amount={amount} bps={bps}");
            }
        }
    }

    #[test]
    fn portion_gives_dust_to_maintainer() {
        assert_eq!(portion(1, 5_000), Ok(0));
        assert_eq!(portion(3, 5_000), Ok(1));
        assert_eq!(portion(100, 5_000), Ok(50));
        assert_eq!(portion(100, 0), Ok(0));
        assert_eq!(portion(100, 10_000), Ok(100));
        assert_eq!(portion(BIG, 10_000), Ok(BIG));
        assert_eq!(portion(i128::MAX, 10_000), Err(Error::Overflow));
    }
}
