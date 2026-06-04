use anyhow::{anyhow, Context};
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

const DEFAULT_NODE_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_CHAIN_ID: &str = "entropis-testnet";
const DEFAULT_L2_NAME: &str = "Entropis";
const DEFAULT_TOKEN_NAME: &str = "Entropis";
const DEFAULT_TOKEN_SYMBOL: &str = "ENT";
const DEFAULT_TONCENTER_TESTNET: &str = "https://testnet.toncenter.com/api/v3";
const DEFAULT_TONAPI_TESTNET: &str = "https://testnet.tonapi.io";
const DEFAULT_CHALLENGE_WINDOW_SEC: u32 = 300;
const DEFAULT_ENT_FAUCET_AMOUNT: u128 = 1_000;
const DEFAULT_ENT_DECIMALS: u8 = 9;
const DEFAULT_ENT_LOGO_PATH: &str = "assets/entropis.png";
const DEFAULT_ENT_FAUCET_REQUIRE_ADMIN: bool = true;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TonNetwork {
    Testnet,
}

#[derive(Clone, Eq, PartialEq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: String) -> anyhow::Result<Self> {
        let value = value.trim().to_owned();
        if value.is_empty() {
            return Err(anyhow!("secret value must not be empty"));
        }
        Ok(Self(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("\"<redacted>\"")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct NodeConfig {
    pub l2_name: String,
    pub chain_id: String,
    pub native_token_name: String,
    pub native_token_symbol: String,
    pub node_addr: SocketAddr,
    pub ton_network: TonNetwork,
    pub toncenter_v3_base_url: String,
    pub toncenter_api_key: SecretString,
    pub tonapi_base_url: String,
    pub tonapi_key: SecretString,
    pub database_url: SecretString,
    pub redis_url: SecretString,
    pub admin_token: SecretString,
    pub challenge_window_sec: u32,
    pub ent_faucet_amount: u128,
    pub ent_decimals: u8,
    pub ent_logo_path: PathBuf,
    pub ent_faucet_require_admin: bool,
}

impl NodeConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let _ = dotenvy::from_filename(".env.local");
        let _ = dotenvy::dotenv();
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> anyhow::Result<Self> {
        let l2_name = optional(&mut lookup, "L2_NAME", DEFAULT_L2_NAME);
        let chain_id = optional(&mut lookup, "L2_CHAIN_ID", DEFAULT_CHAIN_ID);
        let native_token_name = optional(&mut lookup, "L2_NATIVE_TOKEN_NAME", DEFAULT_TOKEN_NAME);
        let native_token_symbol =
            optional(&mut lookup, "L2_NATIVE_TOKEN_SYMBOL", DEFAULT_TOKEN_SYMBOL);
        let node_addr = optional(&mut lookup, "L2_NODE_ADDR", DEFAULT_NODE_ADDR)
            .parse()
            .context("invalid L2_NODE_ADDR")?;
        let ton_network = parse_network(&required(&mut lookup, "TON_NETWORK")?)?;
        let toncenter_v3_base_url = optional(
            &mut lookup,
            "TONCENTER_V3_BASE_URL",
            DEFAULT_TONCENTER_TESTNET,
        );
        let tonapi_base_url = optional(&mut lookup, "TONAPI_BASE_URL", DEFAULT_TONAPI_TESTNET);
        let challenge_window_sec = parse_u32(
            &optional(
                &mut lookup,
                "L2_CHALLENGE_WINDOW_SEC",
                &DEFAULT_CHALLENGE_WINDOW_SEC.to_string(),
            ),
            "L2_CHALLENGE_WINDOW_SEC",
        )?;
        let ent_faucet_amount = parse_u128(
            &optional(
                &mut lookup,
                "ENT_FAUCET_AMOUNT",
                &DEFAULT_ENT_FAUCET_AMOUNT.to_string(),
            ),
            "ENT_FAUCET_AMOUNT",
        )?;
        let ent_decimals = parse_u8(
            &optional(
                &mut lookup,
                "ENT_DECIMALS",
                &DEFAULT_ENT_DECIMALS.to_string(),
            ),
            "ENT_DECIMALS",
        )?;
        let ent_logo_path = PathBuf::from(optional(
            &mut lookup,
            "ENT_LOGO_PATH",
            DEFAULT_ENT_LOGO_PATH,
        ));
        let ent_faucet_require_admin = parse_bool(
            &optional(
                &mut lookup,
                "ENT_FAUCET_REQUIRE_ADMIN",
                bool_literal(DEFAULT_ENT_FAUCET_REQUIRE_ADMIN),
            ),
            "ENT_FAUCET_REQUIRE_ADMIN",
        )?;

        let config = Self {
            l2_name,
            chain_id,
            native_token_name,
            native_token_symbol,
            node_addr,
            ton_network,
            toncenter_v3_base_url,
            toncenter_api_key: SecretString::new(required(&mut lookup, "TONCENTER_API_KEY")?)?,
            tonapi_base_url,
            tonapi_key: SecretString::new(required(&mut lookup, "TONAPI_KEY")?)?,
            database_url: SecretString::new(required(&mut lookup, "DATABASE_URL")?)?,
            redis_url: SecretString::new(required(&mut lookup, "REDIS_URL")?)?,
            admin_token: SecretString::new(required(&mut lookup, "L2_ADMIN_TOKEN")?)?,
            challenge_window_sec,
            ent_faucet_amount,
            ent_decimals,
            ent_logo_path,
            ent_faucet_require_admin,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.ton_network != TonNetwork::Testnet {
            return Err(anyhow!("only TON testnet is allowed for this node"));
        }
        if self.chain_id != DEFAULT_CHAIN_ID {
            return Err(anyhow!("L2_CHAIN_ID must be {DEFAULT_CHAIN_ID}"));
        }
        if !self.toncenter_v3_base_url.contains("testnet.toncenter.com") {
            return Err(anyhow!("TONCENTER_V3_BASE_URL must point to TON testnet"));
        }
        if !self.tonapi_base_url.contains("testnet.tonapi.io") {
            return Err(anyhow!("TONAPI_BASE_URL must point to TON testnet"));
        }
        if !self.database_url.expose().starts_with("postgresql://") {
            return Err(anyhow!("DATABASE_URL must be a PostgreSQL URL"));
        }
        if !self.redis_url.expose().starts_with("redis://") {
            return Err(anyhow!("REDIS_URL must be a Redis URL"));
        }
        if self.admin_token.expose().len() < 16 {
            return Err(anyhow!("L2_ADMIN_TOKEN must be at least 16 bytes"));
        }
        if self.native_token_symbol != DEFAULT_TOKEN_SYMBOL {
            return Err(anyhow!(
                "L2_NATIVE_TOKEN_SYMBOL must be {DEFAULT_TOKEN_SYMBOL}"
            ));
        }
        if self.ent_faucet_amount == 0 {
            return Err(anyhow!("ENT_FAUCET_AMOUNT must be non-zero"));
        }
        if self.ent_decimals != DEFAULT_ENT_DECIMALS {
            return Err(anyhow!("ENT_DECIMALS must be {DEFAULT_ENT_DECIMALS}"));
        }
        if !path_exists_in_cwd_or_ancestors(&self.ent_logo_path) {
            return Err(anyhow!("ENT_LOGO_PATH must point to an existing file"));
        }
        if !self.ent_faucet_require_admin {
            return Err(anyhow!("ENT_FAUCET_REQUIRE_ADMIN must be true for MVP"));
        }
        Ok(())
    }
}

impl fmt::Debug for NodeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NodeConfig")
            .field("l2_name", &self.l2_name)
            .field("chain_id", &self.chain_id)
            .field("native_token_name", &self.native_token_name)
            .field("native_token_symbol", &self.native_token_symbol)
            .field("node_addr", &self.node_addr)
            .field("ton_network", &self.ton_network)
            .field("toncenter_v3_base_url", &self.toncenter_v3_base_url)
            .field("toncenter_api_key", &self.toncenter_api_key)
            .field("tonapi_base_url", &self.tonapi_base_url)
            .field("tonapi_key", &self.tonapi_key)
            .field("database_url", &self.database_url)
            .field("redis_url", &self.redis_url)
            .field("admin_token", &self.admin_token)
            .field("challenge_window_sec", &self.challenge_window_sec)
            .field("ent_faucet_amount", &self.ent_faucet_amount)
            .field("ent_decimals", &self.ent_decimals)
            .field("ent_logo_path", &self.ent_logo_path)
            .field("ent_faucet_require_admin", &self.ent_faucet_require_admin)
            .finish()
    }
}

fn parse_network(value: &str) -> anyhow::Result<TonNetwork> {
    match value {
        "testnet" => Ok(TonNetwork::Testnet),
        _ => Err(anyhow!("TON_NETWORK must be testnet")),
    }
}

fn optional(lookup: &mut impl FnMut(&str) -> Option<String>, key: &str, default: &str) -> String {
    lookup(key)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn required(lookup: &mut impl FnMut(&str) -> Option<String>, key: &str) -> anyhow::Result<String> {
    lookup(key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("{key} is required"))
}

fn parse_u32(value: &str, key: &str) -> anyhow::Result<u32> {
    value
        .parse::<u32>()
        .with_context(|| format!("{key} must be an unsigned 32-bit integer"))
}

fn parse_u128(value: &str, key: &str) -> anyhow::Result<u128> {
    value
        .parse::<u128>()
        .with_context(|| format!("{key} must be an unsigned 128-bit integer"))
}

fn parse_u8(value: &str, key: &str) -> anyhow::Result<u8> {
    value
        .parse::<u8>()
        .with_context(|| format!("{key} must be an unsigned 8-bit integer"))
}

fn parse_bool(value: &str, key: &str) -> anyhow::Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(anyhow!("{key} must be true or false")),
    }
}

fn bool_literal(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn path_exists_in_cwd_or_ancestors(path: &PathBuf) -> bool {
    if path.is_absolute() {
        return path.is_file();
    }

    let Ok(mut current) = std::env::current_dir() else {
        return false;
    };
    loop {
        if current.join(path).is_file() {
            return true;
        }
        if !current.pop() {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn valid_env() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("L2_NAME".to_owned(), "Entropis".to_owned()),
            ("L2_CHAIN_ID".to_owned(), "entropis-testnet".to_owned()),
            ("L2_NATIVE_TOKEN_NAME".to_owned(), "Entropis".to_owned()),
            ("L2_NATIVE_TOKEN_SYMBOL".to_owned(), "ENT".to_owned()),
            ("TON_NETWORK".to_owned(), "testnet".to_owned()),
            (
                "TONCENTER_V3_BASE_URL".to_owned(),
                DEFAULT_TONCENTER_TESTNET.to_owned(),
            ),
            (
                "TONCENTER_API_KEY".to_owned(),
                "toncenter-secret-key".to_owned(),
            ),
            (
                "TONAPI_BASE_URL".to_owned(),
                DEFAULT_TONAPI_TESTNET.to_owned(),
            ),
            ("TONAPI_KEY".to_owned(), "tonapi-secret-key".to_owned()),
            (
                "DATABASE_URL".to_owned(),
                "postgresql://user:pass@localhost:5432/l2".to_owned(),
            ),
            (
                "REDIS_URL".to_owned(),
                "redis://default:pass@localhost:6379".to_owned(),
            ),
            ("L2_ADMIN_TOKEN".to_owned(), "admin-secret-token".to_owned()),
            ("ENT_DECIMALS".to_owned(), "9".to_owned()),
            ("ENT_LOGO_PATH".to_owned(), "assets/entropis.png".to_owned()),
            ("ENT_FAUCET_REQUIRE_ADMIN".to_owned(), "true".to_owned()),
        ])
    }

    fn load_from(map: &BTreeMap<String, String>) -> anyhow::Result<NodeConfig> {
        NodeConfig::from_lookup(|key| map.get(key).cloned())
    }

    #[test]
    fn valid_entropis_testnet_config_loads() {
        let config = load_from(&valid_env()).expect("config");

        assert_eq!(config.l2_name, "Entropis");
        assert_eq!(config.chain_id, "entropis-testnet");
        assert_eq!(config.native_token_symbol, "ENT");
        assert_eq!(config.ton_network, TonNetwork::Testnet);
        assert_eq!(config.ent_decimals, 9);
        assert_eq!(config.ent_logo_path, PathBuf::from("assets/entropis.png"));
        assert!(config.ent_faucet_require_admin);
    }

    #[test]
    fn config_rejects_mainnet_or_wrong_endpoints() {
        let mut env = valid_env();
        env.insert("TON_NETWORK".to_owned(), "mainnet".to_owned());
        assert!(load_from(&env).is_err());

        let mut env = valid_env();
        env.insert(
            "TONCENTER_V3_BASE_URL".to_owned(),
            "https://toncenter.com/api/v3".to_owned(),
        );
        assert!(load_from(&env).is_err());
    }

    #[test]
    fn config_rejects_missing_or_invalid_secrets() {
        let mut env = valid_env();
        env.remove("DATABASE_URL");
        assert!(load_from(&env).is_err());

        let mut env = valid_env();
        env.insert("L2_ADMIN_TOKEN".to_owned(), "short".to_owned());
        assert!(load_from(&env).is_err());
    }

    #[test]
    fn config_rejects_invalid_ent_metadata() {
        let mut env = valid_env();
        env.insert("ENT_DECIMALS".to_owned(), "6".to_owned());
        assert!(load_from(&env).is_err());

        let mut env = valid_env();
        env.insert(
            "ENT_LOGO_PATH".to_owned(),
            "assets/missing-ent.png".to_owned(),
        );
        assert!(load_from(&env).is_err());

        let mut env = valid_env();
        env.insert("ENT_FAUCET_REQUIRE_ADMIN".to_owned(), "false".to_owned());
        assert!(load_from(&env).is_err());
    }

    #[test]
    fn debug_output_redacts_secrets() {
        let env = valid_env();
        let config = load_from(&env).expect("config");
        let debug = format!("{config:?}");

        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(env.get("TONAPI_KEY").unwrap()));
        assert!(!debug.contains(env.get("TONCENTER_API_KEY").unwrap()));
        assert!(!debug.contains(env.get("DATABASE_URL").unwrap()));
        assert!(!debug.contains(env.get("REDIS_URL").unwrap()));
        assert!(!debug.contains(env.get("L2_ADMIN_TOKEN").unwrap()));
    }
}
