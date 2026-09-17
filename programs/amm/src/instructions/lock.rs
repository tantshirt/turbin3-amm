use crate::state::Config;
use anchor_lang::prelude::*;

// same two accounts for lock and unlock. has_one is the whole auth check
#[derive(Accounts)]
pub struct Lock<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
    )]
    pub config: Account<'info, Config>,
}

impl<'info> Lock<'info> {
    pub fn lock(&mut self) -> Result<()> {
        self.config.locked = true;
        Ok(())
    }

    pub fn unlock(&mut self) -> Result<()> {
        self.config.locked = false;
        Ok(())
    }
}
