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

pub fn display_balance(atoms: u64, decimals: usize) -> String {
    let atoms_str = atoms.to_string();
    let len = atoms_str.len();
    let mut result = String::with_capacity(len + len / 3 + 1);

    if len > decimals {
        let decimal_pos = len - decimals;
        let before_decimal = &atoms_str[..decimal_pos];
        let after_decimal = &atoms_str[decimal_pos..];

        // Insert commas every three digits from the right
        for (i, ch) in before_decimal.chars().enumerate() {
            if i > 0 && (before_decimal.len() - i) % 3 == 0 {
                result.push(',');
            }
            result.push(ch);
        }

        // Append the decimal part
        result.push('.');
        result.push_str(after_decimal);
    } else {
        // If all digits are part of the decimal
        let zeros_to_prepend = decimals - len;
        result.push_str("0.");
        for _ in 0..zeros_to_prepend {
            result.push('0');
        }
        result.push_str(&atoms_str);
    }

    result
}

#[inline(always)]
pub fn insert_newlines(s: &str, n: usize) -> String {
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
