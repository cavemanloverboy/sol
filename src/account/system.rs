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
        let tokenkeg_accounts = client
            .get_token_accounts_by_owner(key, TokenAccountsFilter::ProgramId(spl_token::ID))
            .await
            .unwrap()
            .into_iter()
            .map(parse_keyed_account_to_token);

        // Check if this account has token22 accounts
        let token22_accounts = client
            .get_token_accounts_by_owner(key, TokenAccountsFilter::ProgramId(spl_token_2022::ID))
            .await
            .unwrap()
            .into_iter()
            .map(parse_keyed_account_to_token);

        // Collect all accounts
        let token_accounts = tokenkeg_accounts.chain(token22_accounts).collect();

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
                meta_or_mint,
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
