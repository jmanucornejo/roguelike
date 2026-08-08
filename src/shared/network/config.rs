use std::{env, net::SocketAddr};

pub const DEFAULT_GAME_SERVER_ADDR: &str = "127.0.0.1:42069";
pub const DEFAULT_SERVER_BIND_ADDR: &str = "127.0.0.1:42069";
pub const DEFAULT_TOKEN_BIND_ADDR: &str = "127.0.0.1:8080";
pub const DEFAULT_MAX_CLIENTS: usize = 128;

pub fn socket_addr_from_env(name: &str, default: &str) -> Result<SocketAddr, String> {
    let value = env::var(name).unwrap_or_else(|_| default.to_string());
    value.parse().map_err(|error| {
        format!("{name} must be an IP:port socket address, got '{value}': {error}")
    })
}

pub fn usize_from_env(name: &str, default: usize) -> Result<usize, String> {
    let Ok(value) = env::var(name) else {
        return Ok(default);
    };
    value
        .parse::<usize>()
        .map_err(|error| format!("{name} must be a positive integer, got '{value}': {error}"))
        .and_then(|parsed| {
            if parsed == 0 {
                Err(format!("{name} must be greater than zero"))
            } else {
                Ok(parsed)
            }
        })
}

pub fn private_key_from_env(name: &str) -> Result<Option<[u8; 32]>, String> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => parse_private_key(value.trim()).map(Some),
        _ => Ok(None),
    }
}

fn parse_private_key(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err(
            "NETCODE_PRIVATE_KEY must contain exactly 64 hexadecimal characters (32 bytes)"
                .to_string(),
        );
    }

    let mut key = [0_u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16).map_err(|_| {
            "NETCODE_PRIVATE_KEY must contain only hexadecimal characters".to_string()
        })?;
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_32_byte_hex_private_key() {
        let key = parse_private_key(&"ab".repeat(32)).unwrap();
        assert_eq!(key, [0xab; 32]);
    }

    #[test]
    fn rejects_private_keys_with_the_wrong_length() {
        assert!(parse_private_key("abcd").is_err());
    }

    #[test]
    fn rejects_non_hex_private_keys() {
        assert!(parse_private_key(&"zz".repeat(32)).is_err());
    }
}
