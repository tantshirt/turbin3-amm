use crate::error::AmmError;
use crate::state::Config;
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

// ten accounts in here. Boxed so try_accounts doesnt blow the 4KB SBF stack
#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,

    #[account(mint::token_program = token_program)]
    pub mint_x: Box<InterfaceAccount<'info, Mint>>,

    // no same-mint check on purpose. if x == y, vault_x and vault_y derive to the same
    // ATA and the second init fails, so the tx cant go through anyway
    #[account(mint::token_program = token_program)]
    pub mint_y: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = initializer,
        seeds = [b"config", seed.to_le_bytes().as_ref()],
        bump,
        space = 8 + Config::INIT_SPACE,
    )]
    pub config: Box<Account<'info, Config>>,

    // config is the mint authority, so only deposit and withdraw can change LP supply
    #[account(
        init,
        payer = initializer,
        seeds = [b"lp", config.key().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = config,
        mint::token_program = token_program,
    )]
    pub mint_lp: Box<InterfaceAccount<'info, Mint>>,

    // empty PDA that just owns the two treasury token accounts. an ATA is one per
    // owner per mint, so config cant own both a vault and a treasury for the same token
    #[account(
        seeds = [b"treasury", config.key().as_ref()],
        bump,
    )]
    pub treasury: SystemAccount<'info>,

    #[account(
        init,
        payer = initializer,
        associated_token::mint = mint_x,
        associated_token::authority = config,
        associated_token::token_program = token_program,
    )]
    pub vault_x: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = initializer,
        associated_token::mint = mint_y,
        associated_token::authority = config,
        associated_token::token_program = token_program,
    )]
    pub vault_y: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = initializer,
        associated_token::mint = mint_x,
        associated_token::authority = treasury,
        associated_token::token_program = token_program,
    )]
    pub treasury_x: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = initializer,
        associated_token::mint = mint_y,
        associated_token::authority = treasury,
        associated_token::token_program = token_program,
    )]
    pub treasury_y: Box<InterfaceAccount<'info, TokenAccount>>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> Initialize<'info> {
    pub fn initialize(
        &mut self,
        seed: u64,
        fee: u16,
        protocol_fee: u16,
        bumps: &InitializeBumps,
    ) -> Result<()> {
        // strictly under 100 percent. at exactly 10000 bps every swap would net to zero
        require!((fee as u32) + (protocol_fee as u32) < 10_000, AmmError::InvalidFee);

        self.config.set_inner(Config {
            seed,
            authority: self.initializer.key(),
            mint_x: self.mint_x.key(),
            mint_y: self.mint_y.key(),
            fee,
            protocol_fee,
            locked: false,
            config_bump: bumps.config,
            lp_bump: bumps.mint_lp,
            treasury_bump: bumps.treasury,
        });
        Ok(())
    }
}
