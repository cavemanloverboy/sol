use std::str::FromStr;

use clap::Parser;

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

    /// The url/endpoint to use for any rpc requests.
    #[arg(
        long,
        short = 'u',
        default_value = "http://api.mainnet-beta.solana.com",
        global = true
    )]
    rpc_url: String,
}

#[derive(Debug, Parser, Clone)]
pub enum Command {
    /// Provide a transaction signature to inspect status, accounts, logs.
    Transaction(Transaction),

    /// Provide an account pubkey to inspect account contents
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

    match args.command {
        Command::Transaction(transaction) => transaction::handler(args.rpc_url, transaction).await,
        Command::Account(account) => account::handler(args.rpc_url, account).await,
    }
}
