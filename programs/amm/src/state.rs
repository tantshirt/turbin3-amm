use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub seed: u64,
    // the only key that can lock, unlock, or pull fees out of the treasury
    pub authority: Pubkey,
    pub mint_x: Pubkey,
    pub mint_y: Pubkey,
    // both in basis points. fee stays in the vaults for LPs, protocol_fee goes to the treasury
    pub fee: u16,
    pub protocol_fee: u16,
    pub locked: bool,
    pub config_bump: u8,
    pub lp_bump: u8,
    pub treasury_bump: u8,
}
