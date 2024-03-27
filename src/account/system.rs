use std::{cmp::Ordering, str::FromStr};

use futures::future::join_all;
use prettytable::{format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR, row, Table};
use solana_account_decoder::UiAccountData;
use solana_client::{
    nonblocking::rpc_client::RpcClient as Client, rpc_request::TokenAccountsFilter,
    rpc_response::RpcKeyedAccount,
};
use solana_sdk::{account::Account, pubkey::Pubkey, system_program};

use crate::utils::display_balance;

use super::{token::TokenAccountBalance, ParsedAccount};

pub struct SystemAccount<'a> {
    pub account: &'a Account,
    pub key: &'a Pubkey,
    pub token_accounts: Vec<TokenAccountBalance>,
}

impl<'a> SystemAccount<'a> {
    pub async fn parse(
        account: &'a Account,
        key: &'a Pubkey,
        client: &Client,
    ) -> Option<ParsedAccount<'a>> {
        if account.owner != system_program::ID {
            return None;
        }

        // Check if this account has tokenkeg accounts
        let tokenkeg_accounts_futures = client
            .get_token_accounts_by_owner(key, TokenAccountsFilter::ProgramId(spl_token::ID))
            .await
            .unwrap()
            .into_iter()
            .map(parse_keyed_account_to_token)
            .map(|account| async move { get_symbol_for_token_account(&account, &client).await });

        let tokenkeg_accounts = join_all(tokenkeg_accounts_futures).await;

        // Check if this account has token22 accounts
        let token22_accounts_futures = client
            .get_token_accounts_by_owner(key, TokenAccountsFilter::ProgramId(spl_token_2022::ID))
            .await
            .unwrap()
            .into_iter()
            .map(parse_keyed_account_to_token)
            .map(|account| async move { get_symbol_for_token_account(&account, &client).await });

        let token22_accounts = join_all(token22_accounts_futures).await;

        // Collect all accounts
        let mut token_accounts: Vec<TokenAccountBalance> = tokenkeg_accounts
            .iter()
            .chain(token22_accounts.iter())
            .cloned()
            .collect();

        // Sort tokens by symbol
        token_accounts.sort_by(|a, b| match (&a.symbol, &b.symbol) {
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(a_sym), Some(b_sym)) => a_sym.cmp(b_sym),
            (None, None) => Ordering::Equal,
        });

        Some(ParsedAccount::System(SystemAccount {
            account,
            key,
            token_accounts,
        }))
    }

    pub fn display(self) {
        // SOL balance as string in decimal
        let sol_balance: String = display_balance(self.account.lamports, 9);

        let mut account_table = Table::new();
        account_table.add_row(row![c->format!("Account {}", self.key)]);
        account_table.add_row(row!["SOL balance", sol_balance]);

        let mut token_account_table = Table::new();
        token_account_table
            .add_row(row![c->"Token Account", c->"Token", c->"Balance", c->"Standard"]);
        for balance in self.token_accounts {
            let meta_or_mint = balance.mint;
            token_account_table.add_row(row![
                balance.key,
                if let Some(symbol) = balance.symbol {
                    symbol
                } else {
                    meta_or_mint
                },
                balance.balance,
                balance.program
            ]);
        }

        // Print the tables to stdout
        let mut table_of_tables = Table::new();
        table_of_tables.add_row(row![c->account_table]);
        table_of_tables.add_row(row![c->token_account_table]);
        table_of_tables.set_format(*FORMAT_NO_BORDER_LINE_SEPARATOR);
        table_of_tables.printstd();
    }
}

// RPC should have validated so this should be infallible
fn parse_keyed_account_to_token(keyed_account: RpcKeyedAccount) -> TokenAccountBalance {
    // Get account data
    match keyed_account.account.data {
        UiAccountData::Json(json) => {
            TokenAccountBalance::parse_validated_json(json, keyed_account.pubkey)
        }
        _ => unimplemented!("unused right now"),
    }
}

// Helper function to fetch symbol for given token account
async fn get_symbol_for_token_account(
    account: &TokenAccountBalance,
    client: &Client,
) -> TokenAccountBalance {
    let meta_or_mint: String = account.mint.to_string();

    let mint_acc_key: Pubkey = match Pubkey::from_str(&meta_or_mint) {
        Ok(key) => key,
        Err(_) => return account.clone(), // or handle the error appropriately
    };

    let mpl_metadata_key = mpl_token_metadata::accounts::Metadata::find_pda(&mint_acc_key).0;

    let symbol = client
        .get_account_data(&mpl_metadata_key)
        .await
        .map(|data| {
            let metadata = mpl_token_metadata::accounts::Metadata::from_bytes(&data);

            metadata.unwrap().symbol
        })
        .ok();

    TokenAccountBalance {
        symbol,
        ..account.clone()
    }
}
