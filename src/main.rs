use std::str::FromStr;

use clap::Parser;

use solana_cli_config::{Config, CONFIG_FILE};
use solana_sdk::{pubkey::Pubkey, signature::Signature};

mod account;
mod transaction;
mod utils;

/// A command line explorer for the Solana blockchain! Inspect transactions
/// and accounts with this explorer!
#[derive(Debug, Parser)]
#[clap(name = "solana command line explorer", author, version)]
pub struct ExplorerCli {
    #[command(subcommand)]
    command: Command,

    /// Optionally include your RPC endpoint. Use "local", "dev", "main" for default endpoints. Defaults to your Solana CLI config file.
    #[clap(
        long,
        short = 'u',
        default_value = "http://api.mainnet-beta.solana.com",
        global = true
    )]
    rpc_url: Option<String>,
}

pub fn get_network(network_str: &str) -> &str {
    match network_str {
        "devnet" | "dev" | "d" => "https://api.devnet.solana.com",
        "mainnet" | "main" | "m" | "mainnet-beta" => "https://api.mainnet-beta.solana.com",
        "localnet" | "localhost" | "l" | "local" => "http://localhost:8899",
        _ => network_str,
    }
}

#[derive(Debug, Parser, Clone)]
pub enum Command {
    /// Provide a transaction signature to inspect status, accounts, logs.
    Transaction(Transaction),

    /// Provide an account pubkey to inspect account contents (to be implemented)
    Account(Account),
}

#[derive(Debug, Parser, Clone)]
pub struct Transaction {
    /// Signature (base58) of the transaction to inspect
    #[clap(value_parser = Signature::from_str)]
    signature: Signature,
}

#[derive(Debug, Parser, Clone)]
pub struct Account {
    /// Public key (base58) of the account to inspect
    #[clap(value_parser = Pubkey::from_str)]
    pubkey: Pubkey,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = ExplorerCli::parse();
    let config = match CONFIG_FILE.as_ref() {
        Some(config_file) => Config::load(config_file).unwrap_or_else(|_| {
            println!("Failed to load config file: {}", config_file);
            Config::default()
        }),
        None => Config::default(),
    };
    let network_url = &get_network(&args.rpc_url.unwrap_or(config.json_rpc_url)).to_string();

    match args.command {
        Command::Transaction(transaction) => {
            transaction::handler(network_url.to_string(), transaction).await
        }
        Command::Account(account) => account::handler(network_url.to_string(), account).await,
    }
}
