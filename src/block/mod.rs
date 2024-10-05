use std::{cmp::Reverse, collections::BTreeMap};

use num_format::{Locale, ToFormattedString};
use prettytable::{format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR, row, Table};
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcBlockConfig};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{
    RewardType, TransactionDetails, UiConfirmedBlock, UiTransactionEncoding,
};

use crate::utils::get_network;

pub async fn handler(rpc_url: String, block: crate::Block) {
    // Build RPC Client
    let client = RpcClient::new(get_network(&rpc_url));

    'slots: for slot in block.start..=block.end.unwrap_or(block.start) {
        for attempt in 1..=5 {
            // Fetch block
            const BLOCK_CONFIG: RpcBlockConfig = RpcBlockConfig {
                encoding: Some(UiTransactionEncoding::Base64),
                transaction_details: Some(TransactionDetails::Full),
                rewards: Some(true),
                commitment: None,
                max_supported_transaction_version: Some(0),
            };
            let Ok(fetched_block) = client.get_block_with_config(slot, BLOCK_CONFIG).await else {
                println!("failed to fetch block, attempt {attempt}/5");
                continue;
            };

            let parsed_block = ParsedBlock::new(&fetched_block);

            println!();

            use terminal_size::{terminal_size, Width};
            let size = terminal_size();
            let width = size.map(|(Width(w), _height)| w as usize).unwrap_or(32);
            let padded_width = width.saturating_sub(4);

            let mut program_map = BTreeMap::new();
            let transactions = fetched_block.transactions.unwrap();
            let mut vote = 0;
            let mut nonvote = 0;
            let compute_units: u64 = transactions
                .iter()
                .map(|tx| {
                    let decoded_tx = tx.transaction.decode().unwrap();
                    let ixs = decoded_tx.message.instructions();
                    if ixs.len() == 1
                        && *ixs[0].program_id(decoded_tx.message.static_account_keys())
                            == solana_sdk::vote::program::ID
                    {
                        vote += 1;
                    } else {
                        nonvote += 1;
                    }

                    for ix in ixs {
                        program_map
                            .entry(
                                ix.program_id(decoded_tx.message.static_account_keys())
                                    .clone(),
                            )
                            .and_modify(|c: &mut u64| {
                                *c += 1;
                            })
                            .or_insert(1);
                    }

                    Option::<u64>::from(tx.meta.clone().unwrap().compute_units_consumed).unwrap()
                })
                .sum();

            let mut table_of_tables = Table::new();

            // Header table
            let mut header_table = Table::new();
            header_table.add_row(row![c->"Slot", slot]);
            header_table.add_row(row![c->"Parent Slot", fetched_block.parent_slot]);
            header_table.add_row(row![c->"Leader", &parsed_block.leader]);
            header_table.add_row(row![c->"Rewards", format!("◎{}.{:09}", parsed_block.rewards, parsed_block.rewards_sub)]);
            header_table.add_row(row![c->"Blockhash", &fetched_block.blockhash]);
            header_table.add_row(
                row![c->"Transactions", format!("{} nonvote + {} vote = {} total", nonvote, vote, transactions.len())],
            );
            header_table
                .add_row(row![c->"Compute Units", compute_units.to_formatted_string(&Locale::en)]);
            table_of_tables.add_row(row![c->header_table]);

            // Program table
            if block.verbose {
                let mut program_table = Table::new();

                let mut program_invocations: Vec<(Pubkey, u64)> =
                    program_map.into_iter().map(|kv| kv).collect();
                program_invocations.sort_by_key(|kv| Reverse(kv.1));

                program_table.add_row(row!["Program", "Top Level Invocations"]);
                for (program, invocations) in program_invocations {
                    program_table
                        .add_row(row![program, invocations.to_formatted_string(&Locale::en)]);
                }

                table_of_tables.add_row(row![" ".repeat(padded_width)]);
                table_of_tables.add_row(row![c->program_table]);
            }

            table_of_tables.set_format(*FORMAT_NO_BORDER_LINE_SEPARATOR);
            table_of_tables.add_row(row![" ".repeat(padded_width)]);

            table_of_tables.printstd();

            println!();
            continue 'slots;
        }
        println!("failed to fetch block");
    }
}

pub struct ParsedBlock {
    pub leader: String,
    pub rewards: i64,
    pub rewards_sub: i64,
}

impl ParsedBlock {
    /// This makes assumptions about the rpc request config that allow for unwraps
    /// (in the absence of malicious rpc)
    pub fn new(fetched_block: &UiConfirmedBlock) -> ParsedBlock {
        let rewards = fetched_block
            .rewards
            .as_ref()
            .unwrap()
            .iter()
            .find(|reward| reward.reward_type == Some(RewardType::Fee))
            .unwrap();

        ParsedBlock {
            leader: rewards.pubkey.clone(),
            rewards: rewards.lamports / 1_000_000_000,
            rewards_sub: rewards.lamports % 1_000_000_000,
        }
    }
}
