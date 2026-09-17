use crate::curve::{fee_amount, swap_out};
use crate::error::AmmError;
use crate::instructions::shared::transfer_tokens;
use crate::state::Config;
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct Swap<'info> {
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
        seeds = [b"treasury", config.key().as_ref()],
        bump = config.treasury_bump,
    )]
    pub treasury: SystemAccount<'info>,

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

    // both init_if_needed since is_x decides which one is the output side
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

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> Swap<'info> {
    // is_x true means the user is paying x and getting y
    pub fn swap(&mut self, is_x: bool, amount_in: u64, min_out: u64) -> Result<()> {
        require!(!self.config.locked, AmmError::PoolLocked);
        require!(amount_in > 0, AmmError::InvalidAmount);
        require!(
            self.vault_x.amount > 0 && self.vault_y.amount > 0,
            AmmError::NoLiquidity
        );

        // two cuts off the top. lp_cut stays in the vault, protocol_cut goes to the treasury
        let protocol_cut = fee_amount(amount_in, self.config.protocol_fee)?;
        let lp_cut = fee_amount(amount_in, self.config.fee)?;
        let net = amount_in
            .checked_sub(lp_cut)
            .and_then(|n| n.checked_sub(protocol_cut))
            .ok_or(AmmError::Overflow)?;

        let (user_in, vault_in, treasury_in, mint_in, user_out, vault_out, mint_out) = if is_x {
            (
                &self.user_x,
                &self.vault_x,
                &self.treasury_x,
                &self.mint_x,
                &self.user_y,
                &self.vault_y,
                &self.mint_y,
            )
        } else {
            (
                &self.user_y,
                &self.vault_y,
                &self.treasury_y,
                &self.mint_y,
                &self.user_x,
                &self.vault_x,
                &self.mint_x,
            )
        };

        // only the net amount moves the price. the fees dont count toward k
        let out = swap_out(net, vault_in.amount, vault_out.amount)?;
        require!(out > 0, AmmError::InvalidAmount);
        // >= so min_out is the worst fill the user will accept, not one past it
        require!(out >= min_out, AmmError::SlippageExceeded);

        // user pays the vault everything except the protocol cut, so the lp fee lands in the pool
        transfer_tokens(
            user_in,
            vault_in,
            amount_in - protocol_cut,
            mint_in,
            &self.user.to_account_info(),
            &self.token_program.to_account_info(),
            None,
        )?;

        if protocol_cut > 0 {
            transfer_tokens(
                user_in,
                treasury_in,
                protocol_cut,
                mint_in,
                &self.user.to_account_info(),
                &self.token_program.to_account_info(),
                None,
            )?;
        }

        let seed_bytes = self.config.seed.to_le_bytes();
        let seeds = &[b"config".as_ref(), seed_bytes.as_ref(), &[self.config.config_bump]];
        let signer_seeds = &[&seeds[..]];

        transfer_tokens(
            vault_out,
            user_out,
            out,
            mint_out,
            &self.config.to_account_info(),
            &self.token_program.to_account_info(),
            Some(signer_seeds),
        )
    }
}
