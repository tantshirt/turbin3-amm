use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("fees together have to be under 100 percent")]
    InvalidFee,
    #[msg("amount has to be more than zero")]
    InvalidAmount,
    #[msg("price moved past your limit")]
    SlippageExceeded,
    #[msg("pool is locked, only withdraw works")]
    PoolLocked,
    #[msg("pool is empty")]
    NoLiquidity,
    #[msg("math overflow")]
    Overflow,
}
