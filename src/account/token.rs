//! Parsing token and token22 accounts

use prettytable::{format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR, row, Table};
use solana_client::nonblocking::rpc_client::RpcClient as Client;
use solana_sdk::{account::Account, program_option::COption, program_pack::Pack, pubkey::Pubkey};
use spl_token_2022::extension::{BaseStateWithExtensions, ExtensionType};
use spl_type_length_value::variable_len_pack::VariableLenPack;

use crate::utils::display_balance;

use super::ParsedAccount;

pub enum TokenProgramAccount {
    Tokenkeg(TokenkegAccount),
    Token22(Token22Account),
}

impl TokenProgramAccount {
    pub async fn parse<'a>(account: &'a Account, client: &Client) -> Option<ParsedAccount<'a>> {
        // Check account owner for supported token programs
        if account.owner == spl_token::ID {
            // First try parse tokenkeg token account
            if let Ok(token_account) = spl_token::state::Account::unpack(&account.data) {
                // Fetch mint account
                let mint_account_data = client.get_account_data(&token_account.mint).await.unwrap();
                let mint_account = spl_token::state::Mint::unpack(&mint_account_data).unwrap();

                // Try to fetch metadata
                let mpl_metadata_key =
                    mpl_token_metadata::accounts::Metadata::find_pda(&token_account.mint).0;
                let symbol = client
                    .get_account_data(&mpl_metadata_key)
                    .await
                    .map(|data| {
                        mpl_token_metadata::accounts::Metadata::from_bytes(&data)
                            .unwrap()
                            .symbol
                    })
                    .ok();

                return Some(ParsedAccount::tokenkeg_token(
                    token_account,
                    mint_account,
                    symbol,
                ));
            }

            // Then try parsing tokenkeg mint account
            if let Ok(mint_account) = spl_token::state::Mint::unpack(&account.data) {
                return Some(ParsedAccount::tokenkeg_mint(mint_account));
            }
        } else if account.owner == spl_token_2022::ID {
            // First try parse token22 token account
            if let Ok(token_account) = spl_token_2022::state::Account::unpack(&account.data) {
                // Fetch mint account
                let mint_account_data = client.get_account_data(&token_account.mint).await.unwrap();
                let mint_account = spl_token_2022::extension::StateWithExtensions::<
                    spl_token_2022::state::Mint,
                >::unpack(&mint_account_data)
                .unwrap();

                // Try to fetch metadata
                let mpl_metadata_key =
                    mpl_token_metadata::accounts::Metadata::find_pda(&token_account.mint).0;
                let mut symbol = client
                    .get_account_data(&mpl_metadata_key)
                    .await
                    .map(|data| {
                        mpl_token_metadata::accounts::Metadata::from_bytes(&data)
                            .unwrap()
                            .symbol
                    })
                    .ok();

                // If not mpl, try token-2022
                if symbol.is_none() {
                    use spl_token_metadata_interface::state::TokenMetadata;
                    if let Ok(token_metadata) = mint_account
                        .get_extension_bytes::<TokenMetadata>()
                        .and_then(<TokenMetadata as VariableLenPack>::unpack_from_slice)
                    {
                        symbol.replace(token_metadata.symbol);
                    }
                }

                return Some(ParsedAccount::token22_token(
                    token_account,
                    mint_account.base,
                    symbol,
                ));
            }

            // Then try parsing token22 mint account
            if let Ok(mint_account) = spl_token_2022::extension::StateWithExtensions::<
                spl_token_2022::state::Mint,
            >::unpack(&account.data)
            {
                // Get extensions
                let extensions = mint_account.get_extension_types().unwrap();

                return Some(ParsedAccount::token22_mint(mint_account.base, extensions));
            }
        }

        None
    }

    pub fn display(self, key: &Pubkey) {
        match self {
            TokenProgramAccount::Tokenkeg(account) => match account {
                TokenkegAccount::TokenAccount {
                    token_account,
                    mint_account,
                    symbol,
                } => print_token_account(
                    key,
                    token_account.amount,
                    mint_account.decimals,
                    &token_account.mint,
                    symbol,
                ),
                TokenkegAccount::MintAccount(mint_account) => print_mint_account(
                    key,
                    mint_account.supply,
                    mint_account.decimals,
                    &unwrap_coption_pubkey(mint_account.mint_authority),
                    &unwrap_coption_pubkey(mint_account.freeze_authority),
                    &[],
                ),
            },
            TokenProgramAccount::Token22(account) => match account {
                Token22Account::TokenAccount {
                    token_account,
                    mint_account,
                    symbol,
                } => print_token_account(
                    key,
                    token_account.amount,
                    mint_account.decimals,
                    &token_account.mint,
                    symbol,
                ),
                Token22Account::MintAccount {
                    mint_account,
                    extensions,
                } => print_mint_account(
                    key,
                    mint_account.supply,
                    mint_account.decimals,
                    &unwrap_coption_pubkey(mint_account.mint_authority),
                    &unwrap_coption_pubkey(mint_account.freeze_authority),
                    &extensions,
                ),
            },
        }
    }
}

fn unwrap_coption_pubkey(pubkey: COption<Pubkey>) -> Pubkey {
    match pubkey {
        COption::Some(pubkey) => pubkey,
        COption::None => Pubkey::new_from_array([0; 32]),
    }
}

fn print_token_account(
    key: &Pubkey,
    balance: u64,
    decimals: u8,
    mint: &Pubkey,
    symbol: Option<String>,
) {
    let mut token_account_table = Table::new();
    token_account_table.set_titles(row![c->"Token Account", key]);
    if let Some(s) = symbol {
        token_account_table.add_row(row![c->"Symbol", s]);
    }
    token_account_table.add_row(row![c->"Mint", mint]);
    token_account_table.add_row(row![c->"Balance", display_balance(balance, decimals as usize)]);

    use terminal_size::{terminal_size, Width};
    let size = terminal_size();
    let width = size.map(|(Width(w), _height)| w as usize).unwrap_or(32);
    let padded_width = width.saturating_sub(4);

    let mut tables = Table::new();
    tables.add_row(row![c->token_account_table]);
    tables.add_row(row![" ".repeat(padded_width)]);
    tables.set_format(*FORMAT_NO_BORDER_LINE_SEPARATOR);
    tables.printstd();
}

fn print_mint_account(
    key: &Pubkey,
    supply: u64,
    decimals: u8,
    mint_authority_key: &Pubkey,
    freeze_authority_key: &Pubkey,
    extensions: &[ExtensionType],
) {
    let mut mint_account_table = Table::new();
    mint_account_table.set_titles(row![c->"Mint Account", key]);
    mint_account_table.add_row(row![c->"Decimals", decimals]);
    mint_account_table.add_row(row![c->"Supply", display_balance(supply, decimals as usize)]);
    mint_account_table.add_row(row![c->"Mint Authority", mint_authority_key]);
    mint_account_table.add_row(row![c->"Freeze Authority", freeze_authority_key]);
    for (i, ext) in extensions.into_iter().enumerate() {
        mint_account_table.add_row(row![c->format!("Extension {}", i + 1), format!("{ext:?}")]);
    }

    use terminal_size::{terminal_size, Width};
    let size = terminal_size();
    let width = size.map(|(Width(w), _height)| w as usize).unwrap_or(32);
    let padded_width = width.saturating_sub(4);

    let mut tables = Table::new();
    tables.add_row(row![c->mint_account_table]);
    tables.add_row(row![" ".repeat(padded_width)]);
    tables.set_format(*FORMAT_NO_BORDER_LINE_SEPARATOR);
    tables.printstd();
}

pub enum TokenkegAccount {
    TokenAccount {
        token_account: spl_token::state::Account,
        mint_account: spl_token::state::Mint,
        symbol: Option<String>,
    },
    MintAccount(spl_token::state::Mint),
}

pub enum Token22Account {
    TokenAccount {
        token_account: spl_token_2022::state::Account,
        mint_account: spl_token_2022::state::Mint,
        symbol: Option<String>,
    },
    MintAccount {
        mint_account: spl_token_2022::state::Mint,
        extensions: Vec<ExtensionType>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenAccountBalance {
    pub key: String,
    pub balance: UiAmount,
    pub mint: String,
    pub program: &'static str,
}

type UiAmount = String;

use std::str::FromStr;
macro_rules! from_str {
    ($x:expr) => {
        FromStr::from_str(&$x.as_str().unwrap()).unwrap()
    };
}

impl TokenAccountBalance {
    pub(crate) fn parse_validated_json(
        json: solana_account_decoder::parse_account_data::ParsedAccount,
        key: String,
    ) -> TokenAccountBalance {
        let info = &json.parsed["info"];

        if json.program == "spl-token" {
            TokenAccountBalance {
                key,
                program: "spl-token",
                balance: from_str!(info["tokenAmount"]["uiAmountString"]),
                mint: from_str!(info["mint"]),
            }
        } else if json.program == "spl-token-2022" {
            TokenAccountBalance {
                key,
                program: "spl-token",
                balance: from_str!(info["tokenAmount"]["uiAmountString"]),
                mint: from_str!(info["mint"]),
            }
        } else {
            unimplemented!("scaffolded for other token programs... {}", json.program)
        }
    }
}
