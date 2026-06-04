use crate::config::NodeConfig;
use std::collections::BTreeMap;

pub(crate) fn test_config() -> NodeConfig {
    let env = BTreeMap::from([
        ("L2_NAME".to_owned(), "Entropis".to_owned()),
        ("L2_CHAIN_ID".to_owned(), "entropis-testnet".to_owned()),
        ("L2_NATIVE_TOKEN_NAME".to_owned(), "Entropis".to_owned()),
        ("L2_NATIVE_TOKEN_SYMBOL".to_owned(), "ENT".to_owned()),
        ("TON_NETWORK".to_owned(), "testnet".to_owned()),
        (
            "TONCENTER_V3_BASE_URL".to_owned(),
            "https://testnet.toncenter.com/api/v3".to_owned(),
        ),
        (
            "TONCENTER_API_KEY".to_owned(),
            "test-api-token-a".to_owned(),
        ),
        (
            "TONAPI_BASE_URL".to_owned(),
            "https://testnet.tonapi.io".to_owned(),
        ),
        ("TONAPI_KEY".to_owned(), "test-api-token-b".to_owned()),
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
    ]);
    NodeConfig::from_lookup(|key| env.get(key).cloned()).expect("test config")
}
