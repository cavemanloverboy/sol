pub fn get_network(network_str: &str) -> String {
    match network_str {
        "devnet" | "dev" | "d" => "https://api.devnet.solana.com",
        "testnet" | "test" | "t" => "https://api.testnet.solana.com",
        "mainnet" | "main" | "m" | "mainnet-beta" => "https://api.mainnet-beta.solana.com",
        "localnet" | "localhost" | "l" | "local" => "http://localhost:8899",
        // custom
        _ => network_str,
    }
    .to_string()
}
