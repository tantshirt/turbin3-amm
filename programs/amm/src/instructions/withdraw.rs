use crate::curve::withdraw_amounts;
use crate::error::AmmError;
use crate::instructions::shared::transfer_tokens;
use crate::state::Config;
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{burn, Burn, Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct Withdraw<'info> {
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
        init_if_needed,
        payer = user,
        associated_token::mint = mint_x,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_x: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint_y,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_y: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_lp,
        associated_token::authority = user,
        associated_token::token_program = token_program,
    )]
    pub user_lp: Box<InterfaceAccount<'info, TokenAccount>>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> Withdraw<'info> {
    // no locked check on purpose. LPs can always get out, thats the whole downtime story
    pub fn withdraw(&mut self, amount: u64, min_x: u64, min_y: u64) -> Result<()> {
        require!(amount > 0, AmmError::InvalidAmount);
        require!(self.mint_lp.supply > 0, AmmError::NoLiquidity);

        let (x, y) = withdraw_amounts(
            amount,
            self.vault_x.amount,
            self.vault_y.amount,
            self.mint_lp.supply,
        )?;
        require!(x >= min_x && y >= min_y, AmmError::SlippageExceeded);

        // burn first. if the user doesnt actually have the LP tokens this is what fails
        burn(
            CpiContext::new(
                self.token_program.key(),
                Burn {
                    mint: self.mint_lp.to_account_info(),
                    from: self.user_lp.to_account_info(),
                    authority: self.user.to_account_info(),
                },
            ),
            amount,
        )?;

        let seed_bytes = self.config.seed.to_le_bytes();
        let seeds = &[b"config".as_ref(), seed_bytes.as_ref(), &[self.config.config_bump]];
        let signer_seeds = &[&seeds[..]];

        transfer_tokens(
            &self.vault_x,
            &self.user_x,
            x,
            &self.mint_x,
            &self.config.to_account_info(),
            &self.token_program.to_account_info(),
            Some(signer_seeds),
        )?;
        transfer_tokens(
            &self.vault_y,
            &self.user_y,
            y,
            &self.mint_y,
            &self.config.to_account_info(),
            &self.token_program.to_account_info(),
            Some(signer_seeds),
        )
    }
}
