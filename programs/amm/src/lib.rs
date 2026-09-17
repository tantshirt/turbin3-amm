pub mod curve;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;
pub use state::*;

declare_id!("8FQjGhvBvs2QW6KEL13gwGwWacfRsGTw7EXPiMvBPedc");

#[program]
pub mod amm {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, seed: u64, fee: u16, protocol_fee: u16) -> Result<()> {
        ctx.accounts.initialize(seed, fee, protocol_fee, &ctx.bumps)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64, max_x: u64, max_y: u64) -> Result<()> {
        ctx.accounts.deposit(amount, max_x, max_y)
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64, min_x: u64, min_y: u64) -> Result<()> {
        ctx.accounts.withdraw(amount, min_x, min_y)
    }

    pub fn swap(ctx: Context<Swap>, is_x: bool, amount_in: u64, min_out: u64) -> Result<()> {
        ctx.accounts.swap(is_x, amount_in, min_out)
    }

    pub fn lock(ctx: Context<Lock>) -> Result<()> {
        ctx.accounts.lock()
    }

    pub fn unlock(ctx: Context<Lock>) -> Result<()> {
        ctx.accounts.unlock()
    }

    pub fn collect_fees(ctx: Context<CollectFees>) -> Result<()> {
        ctx.accounts.collect_fees()
    }
}
