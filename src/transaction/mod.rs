use std::borrow::Cow;

use colored::{ColoredString, Colorize};
use num_format::{Locale, ToFormattedString};
use prettytable::{format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR, row, Table};
use solana_client::{
    nonblocking::rpc_client::RpcClient as Client, rpc_config::RpcTransactionConfig,
};
use solana_sdk::{
    address_lookup_table::state::AddressLookupTable,
    hash::Hash,
    instruction::AccountMeta,
    message::VersionedMessage,
    transaction::{TransactionVersion, VersionedTransaction},
};
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransactionWithStatusMeta,
    UiTransactionEncoding, UiTransactionStatusMeta,
};

use crate::{utils::get_network, Transaction};

pub async fn handler(rpc_url: String, transaction: Transaction) {
    // Build RPC Client
    let client = Client::new(get_network(&rpc_url));

    // Fetch transaction
    let fetched_transaction = client
        .get_transaction_with_config(
            &transaction.signature,
            RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Base58),
                max_supported_transaction_version: Some(0),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // Parse transaction
    let parsed_transaction = parse_transaction(fetched_transaction, &client)
        .await
        .unwrap();

    parsed_transaction.view();
}

async fn parse_transaction(
    transaction: EncodedConfirmedTransactionWithStatusMeta,
    client: &Client,
) -> Option<ParsedTransaction> {
    let EncodedConfirmedTransactionWithStatusMeta {
        slot,
        transaction:
            EncodedTransactionWithStatusMeta {
                transaction: encoded_transaction,
                meta,
                version,
            },
        block_time,
    } = transaction;

    let Some(meta) = meta else {
        // TODO
        return None;
    };

    let Some(time) = block_time else {
        // TODO
        return None;
    };

    let Some(version) = version else {
        // TODO
        return None;
    };

    // Decode transaction
    let VersionedTransaction {
        signatures: _,
        message,
    } = encoded_transaction
        .decode()
        .expect("TODO: failed to decode error");

    // Get accounts
    let accounts = match &message {
        VersionedMessage::Legacy(legacy) => {
            // Legacy only has static accounts
            let accounts = legacy
                .account_keys
                .iter()
                .enumerate()
                .map(|(idx, &account)| {
                    if legacy.is_writable(idx) {
                        AccountMeta::new(account, legacy.is_signer(idx))
                    } else {
                        AccountMeta::new_readonly(account, legacy.is_signer(idx))
                    }
                })
                .collect();

            // Legacy only has static accounts
            accounts
        }

        VersionedMessage::V0(v0) => {
            // Start with static accounts
            let mut accounts: Vec<AccountMeta> = v0
                .account_keys
                .iter()
                .enumerate()
                .map(|(idx, &account)| {
                    if v0.is_maybe_writable(idx) {
                        AccountMeta::new(account, message.is_signer(idx))
                    } else {
                        AccountMeta::new_readonly(account, message.is_signer(idx))
                    }
                })
                .collect();

            // Then, try account lookups
            // (this may fail if lookup table is deactivated and closed)
            if let Some(lookups) = message.address_table_lookups() {
                for lookup in lookups {
                    // Fetch and try deserialize
                    match client
                        .get_account_data(&lookup.account_key)
                        .await
                        .as_deref()
                        .map(AddressLookupTable::deserialize)
                    {
                        // If fetch + deserialize succeeded, perform lookups.
                        // Lookups cannot be signers.
                        Ok(Ok(alt)) => {
                            // Write accounts
                            for &idx in &lookup.writable_indexes {
                                accounts.push(AccountMeta::new(alt.addresses[idx as usize], false))
                            }

                            // Read accounts
                            for &idx in &lookup.readonly_indexes {
                                accounts.push(AccountMeta::new_readonly(
                                    alt.addresses[idx as usize],
                                    false,
                                ))
                            }
                        }

                        e => {
                            println!(
                                "failed to perform lookup for table {}: {e:#?}",
                                lookup.account_key
                            );
                        }
                    }
                }
            }

            accounts
        }
    };

    // First, static accounts
    Some(ParsedTransaction {
        meta,
        time,
        accounts,
        slot,
        version,
        blockhash: *message.recent_blockhash(),
    })
}

pub struct ParsedTransaction {
    meta: UiTransactionStatusMeta,
    accounts: Vec<AccountMeta>,
    blockhash: Hash,
    slot: u64,
    version: TransactionVersion,
    time: i64,
}

impl ParsedTransaction {
    fn view(self) {
        // Create status table
        let mut status_table = Table::new();
        status_table.set_titles(row![
            c-> "Transaction Overview",
        ]);
        let result = if self.meta.status.is_ok() {
            "SUCCESS".green()
        } else {
            "FAILURE".red()
        };
        let cus: u64 = Option::unwrap(self.meta.compute_units_consumed.into());
        status_table.add_row(row!["Result", result]);
        status_table.add_row(row!["Slot", self.slot]);
        status_table.add_row(row!["Timestamp", self.time]);
        status_table.add_row(row!["Fee", format_fee(self.meta.fee)]);
        status_table.add_row(row!["Version", format_version(&self.version)]);
        status_table.add_row(row!["Recent Blockhash", self.blockhash.to_string()]);
        status_table.add_row(row![
            "Compute Units Consumed",
            cus.to_formatted_string(&Locale::en)
        ]);

        // Create accounts table
        let mut accounts_table = Table::new();
        let accounts_iter = self.accounts.iter();
        let pre_balances_iter = self.meta.pre_balances.iter();
        let post_balances_iter = self.meta.post_balances.iter();
        accounts_table.set_titles(row![
            c->"Accounts",
            c->"Signer",
            c->"Writable",
            c->"Pre-Balances",
            c->"Post-balances"
        ]);

        let sgn = |account: &AccountMeta| {
            if account.is_signer {
                "TRUE".green()
            } else {
                "FALSE".red()
            }
        };
        let wrt = |account: &AccountMeta| {
            if account.is_writable {
                "TRUE".green()
            } else {
                "FALSE".red()
            }
        };

        for (account, (pre, post)) in accounts_iter.zip(pre_balances_iter.zip(post_balances_iter)) {
            accounts_table.add_row(row![
                account.pubkey.to_string(),
                sgn(account),
                wrt(account),
                format_pre_post(pre, pre, post),
                format_pre_post(post, pre, post),
            ]);
        }

        // TODO: Token Accounts pre/post
        let mut _token_accounts = Table::new();

        // TODO: Instructions table
        let mut _instructions_table = Table::new();

        // Get terminal size for newlines
        use terminal_size::{terminal_size, Width};
        let size = terminal_size();
        let width = size
            .map(|(Width(w), _height)| w as usize)
            .unwrap_or(32)
            .saturating_sub(6);

        // Create logs table
        let mut logs_table = Table::new();
        logs_table.set_titles(row![c->"Program Logs"]);
        let opt_log_messages: Option<Vec<String>> = self.meta.log_messages.into();
        if let Some(log_msgs) = opt_log_messages {
            for log in log_msgs {
                // let mut rem: &str = &log;
                // let mut curr = rem;
                // while rem.len() > LOG_MAX_WIDTH {
                //     (curr, rem) = rem.split_at(LOG_MAX_WIDTH);
                //     // logs_table.add_row(row)
                // }
                // curr = rem;
                // logs_table.add_row(row![log]);
                logs_table.add_row(row![insert_newlines(&log, width)]);
            }
        }

        // Print the table to stdout
        let mut table_of_tables = Table::new();
        table_of_tables.add_row(row![c->status_table]);
        table_of_tables.add_row(row![c->accounts_table]);
        table_of_tables.add_row(row![c->logs_table]);
        table_of_tables.set_format(*FORMAT_NO_BORDER_LINE_SEPARATOR);
        table_of_tables.printstd();
    }
}

#[inline(always)]
fn insert_newlines(s: &str, n: usize) -> String {
    let mut result = String::new();
    let mut counter = 0;

    for c in s.chars() {
        if counter == n {
            result.push('\n');
            counter = 0;
        }
        result.push(c);
        counter += 1;
    }

    result
}

#[inline(always)]
fn format_fee(fee_lamports: u64) -> String {
    let floating = fee_lamports as f64 / 1e9;
    format!("◎{floating}")
}

#[inline(always)]
fn format_version(version: &TransactionVersion) -> Cow<'static, str> {
    match version {
        TransactionVersion::Legacy(_) => Cow::Borrowed("Legacy"),
        TransactionVersion::Number(n) => Cow::Owned(n.to_string()),
    }
}

#[inline(always)]
fn format_pre_post(current: &u64, pre: &u64, post: &u64) -> ColoredString {
    if pre > post {
        current.to_formatted_string(&Locale::en).red()
    } else if pre < post {
        current.to_formatted_string(&Locale::en).green()
    } else {
        current.to_formatted_string(&Locale::en).into()
    }
}
