use std::io::Write;

use base64::Engine;
use prettytable::{format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR, row, Table};
use solana_client::nonblocking::rpc_client::RpcClient as Client;
use solana_sdk::{account::Account, pubkey::Pubkey};
use spl_token_2022::extension::ExtensionType;

use crate::utils::{display_balance, get_network};

use self::{
    system::SystemAccount,
    token::{Token22Account, TokenProgramAccount, TokenkegAccount},
};

pub mod system;
pub mod token;

pub async fn handler(rpc_url: String, account: crate::Account) {
    // Build RPC Client
    let client = Client::new(get_network(&rpc_url));

    // Fetch account
    let fetched_account: Account = client.get_account(&account.pubkey).await.unwrap();

    // Parse account
    let parsed_account = parse_account(&fetched_account, &account.pubkey, &client).await;

    println!();
    parsed_account.display(&account.pubkey);
    println!();
}

async fn parse_account<'a>(
    account: &'a Account,
    key: &'a Pubkey,
    client: &Client,
) -> ParsedAccount<'a> {
    // First try parse system program
    SystemAccount::parse(account, key, client)
        .await
        // Then try parse token account
        .or(TokenProgramAccount::parse(account, client).await)
        // Finally, fallback (infallible)
        .or_else(|| Some(ParsedAccount::Other(account)))
        .unwrap()
}

pub enum ParsedAccount<'a> {
    System(SystemAccount<'a>),
    TokenProgram(TokenProgramAccount),
    Other(&'a Account),
}

impl<'a> ParsedAccount<'a> {
    pub fn display(self, key: &Pubkey) {
        match self {
            ParsedAccount::System(system) => system.display(),
            ParsedAccount::TokenProgram(token) => token.display(key),
            ParsedAccount::Other(other) => other_display(other, key),
        }
    }
}

fn other_display(other: &Account, key: &Pubkey) {
    let Account {
        lamports,
        data,
        owner,
        executable,
        rent_epoch: _,
    } = other;

    use terminal_size::{terminal_size, Width};
    let size = terminal_size();
    let width = size.map(|(Width(w), _height)| w as usize).unwrap_or(32);
    let padded_width = width.saturating_sub(4);

    // Encode data in base64
    const ACCOUNT_DATA_STR: &str = "Account Data";
    let bs64_approx_max_len = 3 * data.len() / 2;
    let mut data_string = String::with_capacity(2 * width + 2 + bs64_approx_max_len);
    let pad_len = width / 2 - ACCOUNT_DATA_STR.len() / 2;
    data_string.push_str(&" ".repeat(pad_len));
    data_string.push_str(ACCOUNT_DATA_STR);
    data_string.push('\n');
    data_string.push_str(&"-".repeat(width));
    base64::engine::general_purpose::STANDARD.encode_string(data, &mut data_string);
    data_string.push('\n');

    let mut account_table = Table::new();
    account_table.set_titles(row![c->"Account", key]);
    account_table.add_row(row![c->"Owner", owner]);
    account_table.add_row(row![c->"SOL Balance", display_balance(*lamports, 9)]);
    account_table.add_row(row![c->"Executable", executable]);

    let mut tables = Table::new();
    tables.add_row(row![c->account_table]);
    tables.add_row(row![" ".repeat(padded_width)]);
    tables.set_format(*FORMAT_NO_BORDER_LINE_SEPARATOR);
    tables.printstd();

    let mut stdout = std::io::stdout();
    stdout.write(data_string.as_bytes()).unwrap();
    stdout.flush().unwrap();
}

/// Helper declaration methods since direct enum declaration is verbose
impl<'a> ParsedAccount<'a> {
    #[inline(always)]
    pub fn tokenkeg_token(
        token_account: spl_token::state::Account,
        mint_account: spl_token::state::Mint,
        symbol: Option<String>,
    ) -> ParsedAccount<'a> {
        ParsedAccount::TokenProgram(TokenProgramAccount::Tokenkeg(
            TokenkegAccount::TokenAccount {
                token_account,
                mint_account,
                symbol,
            },
        ))
    }

    #[inline(always)]
    pub fn tokenkeg_mint(mint: spl_token::state::Mint) -> ParsedAccount<'a> {
        ParsedAccount::TokenProgram(TokenProgramAccount::Tokenkeg(TokenkegAccount::MintAccount(
            mint,
        )))
    }

    #[inline(always)]
    pub fn token22_token(
        token_account: spl_token_2022::state::Account,
        mint_account: spl_token_2022::state::Mint,
        symbol: Option<String>,
    ) -> ParsedAccount<'a> {
        ParsedAccount::TokenProgram(TokenProgramAccount::Token22(Token22Account::TokenAccount {
            token_account,
            mint_account,
            symbol,
        }))
    }

    #[inline(always)]
    pub fn token22_mint(
        mint_account: spl_token_2022::state::Mint,
        extensions: Vec<ExtensionType>,
    ) -> ParsedAccount<'a> {
        ParsedAccount::TokenProgram(TokenProgramAccount::Token22(Token22Account::MintAccount {
            mint_account,
            extensions,
        }))
    }
}
