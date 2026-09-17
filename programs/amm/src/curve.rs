use crate::error::AmmError;
use anchor_lang::prelude::*;

// all the x*y=k math lives here, no crate. everything goes through u128 so a*b cant overflow

// a * b / c rounded down
fn mul_div(a: u64, b: u64, c: u64) -> Result<u64> {
    let out = (a as u128)
        .checked_mul(b as u128)
        .ok_or(AmmError::Overflow)?
        .checked_div(c as u128)
        .ok_or(AmmError::Overflow)?;
    u64::try_from(out).map_err(|_| AmmError::Overflow.into())
}

// a * b / c rounded up
fn mul_div_up(a: u64, b: u64, c: u64) -> Result<u64> {
    let top = (a as u128).checked_mul(b as u128).ok_or(AmmError::Overflow)?;
    let out = top
        .checked_add(c as u128 - 1)
        .ok_or(AmmError::Overflow)?
        .checked_div(c as u128)
        .ok_or(AmmError::Overflow)?;
    u64::try_from(out).map_err(|_| AmmError::Overflow.into())
}

// lp tokens wanted -> how much x and y that costs. rounds up, against the depositor
pub fn deposit_amounts(lp: u64, reserve_x: u64, reserve_y: u64, supply: u64) -> Result<(u64, u64)> {
    let x = mul_div_up(lp, reserve_x, supply)?;
    let y = mul_div_up(lp, reserve_y, supply)?;
    Ok((x, y))
}

// lp tokens burned -> how much x and y comes out. rounds down, against the withdrawer
pub fn withdraw_amounts(lp: u64, reserve_x: u64, reserve_y: u64, supply: u64) -> Result<(u64, u64)> {
    let x = mul_div(lp, reserve_x, supply)?;
    let y = mul_div(lp, reserve_y, supply)?;
    Ok((x, y))
}

// x*y=k. out = reserve_out * in / (reserve_in + in), rounded down so k never drops
pub fn swap_out(amount_in: u64, reserve_in: u64, reserve_out: u64) -> Result<u64> {
    let new_reserve_in = reserve_in.checked_add(amount_in).ok_or(AmmError::Overflow)?;
    mul_div(reserve_out, amount_in, new_reserve_in)
}

// amount * bps / 10000, rounded down
pub fn fee_amount(amount: u64, bps: u16) -> Result<u64> {
    mul_div(amount, bps as u64, 10_000)
}
