use crate::curve::deposit_amounts;
use crate::error::AmmError;
use crate::instructions::shared::transfer_tokens;
use crate::state::Config;
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{mint_to, Mint, MintTo, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mint::token_program = token_program)]
    pub mint_x: Box<InterfaceAccount<'info, Mint>>,

    #[account(mint::token_program = token_program)]
    pub mint_y: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        has_one = mint_x,
        has_one = mint_y,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [b"lp", config.key().as_ref()],
        bump = config.lp_bump,
        mint::token_program = token_program,
    )]
    pub mint_lp: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = config,
        associated_token::token_program = token_program,
    )]
    pub vault_x: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = config,
        associated_token::token_program = token_program,
    )]
    pub vault_y: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_x: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_y: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint_lp,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_lp: Box<InterfaceAccount<'info, TokenAccount>>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> Deposit<'info> {
    // amount is how many LP tokens the user wants. max_x and max_y cap what theyll pay for them
    pub fn deposit(&mut self, amount: u64, max_x: u64, max_y: u64) -> Result<()> {
        require!(!self.config.locked, AmmError::PoolLocked);
        require!(amount > 0, AmmError::InvalidAmount);

        // first depositor sets the price. after that you pay the pools current ratio
        let (x, y) = if self.mint_lp.supply == 0 {
            (max_x, max_y)
        } else {
            deposit_amounts(
                amount,
                self.vault_x.amount,
                self.vault_y.amount,
                self.mint_lp.supply,
            )?
        };

        require!(x > 0 && y > 0, AmmError::InvalidAmount);
        require!(x <= max_x && y <= max_y, AmmError::SlippageExceeded);

        transfer_tokens(
            &self.user_x,
            &self.vault_x,
            x,
            &self.mint_x,
            &self.user.to_account_info(),
            &self.token_program.to_account_info(),
            None,
        )?;
        transfer_tokens(
            &self.user_y,
            &self.vault_y,
            y,
            &self.mint_y,
            &self.user.to_account_info(),
            &self.token_program.to_account_info(),
            None,
        )?;

        let seed_bytes = self.config.seed.to_le_bytes();
        let seeds = &[b"config".as_ref(), seed_bytes.as_ref(), &[self.config.config_bump]];
        let signer_seeds = &[&seeds[..]];

        mint_to(
            CpiContext::new_with_signer(
                self.token_program.key(),
                MintTo {
                    mint: self.mint_lp.to_account_info(),
                    to: self.user_lp.to_account_info(),
                    authority: self.config.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )
    }
}
