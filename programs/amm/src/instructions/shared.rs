use anchor_lang::prelude::*;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TransferChecked};

// one helper for both directions: pass seeds when a PDA is the authority, None when a user is
pub fn transfer_tokens<'info>(
    from: &InterfaceAccount<'info, TokenAccount>,
    to: &InterfaceAccount<'info, TokenAccount>,
    amount: u64,
    mint: &InterfaceAccount<'info, Mint>,
    authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    let accounts = TransferChecked {
        from: from.to_account_info(),
        mint: mint.to_account_info(),
        to: to.to_account_info(),
        authority: authority.clone(),
    };

    let ctx = match seeds {
        Some(s) => CpiContext::new_with_signer(token_program.key(), accounts, s),
        None => CpiContext::new(token_program.key(), accounts),
    };

    // transfer_checked so the mint and decimals get validated, not just the amount
    transfer_checked(ctx, amount, mint.decimals)
}
