use crate::instructions::shared::transfer_tokens;
use crate::state::Config;
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct CollectFees<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mint::token_program = token_program)]
    pub mint_x: Box<InterfaceAccount<'info, Mint>>,

    #[account(mint::token_program = token_program)]
    pub mint_y: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        has_one = authority,
        has_one = mint_x,
        has_one = mint_y,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        seeds = [b"treasury", config.key().as_ref()],
        bump = config.treasury_bump,
    )]
    pub treasury: SystemAccount<'info>,

    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = treasury,
        associated_token::token_program = token_program,
    )]
    pub treasury_x: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = treasury,
        associated_token::token_program = token_program,
    )]
    pub treasury_y: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = mint_x,
        associated_token::authority = authority,
        associated_token::token_program = token_program,
    )]
    pub authority_x: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = mint_y,
        associated_token::authority = authority,
        associated_token::token_program = token_program,
    )]
    pub authority_y: Box<InterfaceAccount<'info, TokenAccount>>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> CollectFees<'info> {
    // empties both treasury accounts into the authoritys wallet
    pub fn collect_fees(&mut self) -> Result<()> {
        let config_key = self.config.key();
        let seeds = &[
            b"treasury".as_ref(),
            config_key.as_ref(),
            &[self.config.treasury_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        transfer_tokens(
            &self.treasury_x,
            &self.authority_x,
            self.treasury_x.amount,
            &self.mint_x,
            &self.treasury.to_account_info(),
            &self.token_program.to_account_info(),
            Some(signer_seeds),
        )?;
        transfer_tokens(
            &self.treasury_y,
            &self.authority_y,
            self.treasury_y.amount,
            &self.mint_y,
            &self.treasury.to_account_info(),
            &self.token_program.to_account_info(),
            Some(signer_seeds),
        )
    }
}
