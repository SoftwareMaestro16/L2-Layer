use super::*;
use std::collections::BTreeMap;

fn base_env() -> BTreeMap<String, String> {
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
            "test-api-token-a".to_owned(),
        ),
        (
            "TONAPI_BASE_URL".to_owned(),
            DEFAULT_TONAPI_TESTNET.to_owned(),
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
    ])
}

fn load_from(map: &BTreeMap<String, String>) -> anyhow::Result<NodeConfig> {
    NodeConfig::from_lookup(|key| map.get(key).cloned())
}

fn live_env() -> BTreeMap<String, String> {
    let mut env = base_env();
    env.insert("L2_RUNTIME_MODE".to_owned(), "testnet-prototype".to_owned());
    env.insert(
        "L2_DEV_ADMIN_DEPOSITS_ENABLED".to_owned(),
        "false".to_owned(),
    );
    env.insert("L1_VAULT_ADDRESS".to_owned(), "EQvault".to_owned());
    env.insert("L1_ROLLUP_ROOT_ADDRESS".to_owned(), "EQroot".to_owned());
    env.insert(
        "L1_SEQUENCER_SENDER_ADDRESS".to_owned(),
        "EQsequencer".to_owned(),
    );
    env.insert(
        "L1_COMMIT_SIGNER_ENDPOINT".to_owned(),
        "http://127.0.0.1:8800/sign-commit".to_owned(),
    );
    env.insert(
        "L1_COMMIT_SIGNER_TOKEN".to_owned(),
        "test-signer-token".to_owned(),
    );
    env
}

#[test]
fn local_dev_mode_defaults_keep_manual_dev_flow() {
    let config = load_from(&base_env()).expect("local config");

    assert_eq!(config.runtime_mode, RuntimeMode::LocalDev);
    assert!(config.dev_admin_deposits_enabled);
    assert!(!config.l1_deposit_indexer_enabled);
    assert!(!config.l1_batch_relayer_enabled);
}

#[test]
fn testnet_prototype_defaults_enable_live_workers() {
    let config = load_from(&live_env()).expect("live config");

    assert_eq!(config.runtime_mode, RuntimeMode::TestnetPrototype);
    assert!(!config.dev_admin_deposits_enabled);
    assert!(config.l1_deposit_indexer_enabled);
    assert!(config.l1_batch_relayer_enabled);
    assert_eq!(config.l1_vault_address.as_deref(), Some("EQvault"));
    assert_eq!(config.l1_rollup_root_address.as_deref(), Some("EQroot"));
}

#[test]
fn testnet_prototype_rejects_dev_deposits_or_disabled_workers() {
    let mut env = live_env();
    env.insert(
        "L2_DEV_ADMIN_DEPOSITS_ENABLED".to_owned(),
        "true".to_owned(),
    );
    let error = load_from(&env).unwrap_err().to_string();
    assert!(error.contains("L2_DEV_ADMIN_DEPOSITS_ENABLED must be false"));

    let mut env = live_env();
    env.insert("L1_DEPOSIT_INDEXER_ENABLED".to_owned(), "false".to_owned());
    let error = load_from(&env).unwrap_err().to_string();
    assert!(error.contains("L1_DEPOSIT_INDEXER_ENABLED must be true"));

    let mut env = live_env();
    env.insert("L1_BATCH_RELAYER_ENABLED".to_owned(), "false".to_owned());
    let error = load_from(&env).unwrap_err().to_string();
    assert!(error.contains("L1_BATCH_RELAYER_ENABLED must be true"));
}

#[test]
fn testnet_prototype_requires_contracts_and_signer() {
    let mut env = live_env();
    env.remove("L1_VAULT_ADDRESS");
    assert!(load_from(&env)
        .unwrap_err()
        .to_string()
        .contains("L1_VAULT_ADDRESS is required"));

    let mut env = live_env();
    env.remove("L1_COMMIT_SIGNER_TOKEN");
    assert!(load_from(&env)
        .unwrap_err()
        .to_string()
        .contains("L1_COMMIT_SIGNER_TOKEN is required"));
}

#[test]
fn startup_summary_excludes_secret_material() {
    let env = live_env();
    let config = load_from(&env).expect("live config");
    let summary = config.startup_summary();
    let rendered = format!("{summary:?}");

    assert_eq!(summary.runtime_mode, "testnet-prototype");
    assert!(summary.database_configured);
    assert!(summary.redis_configured);
    assert!(summary.l1_commit_signer_endpoint_configured);
    assert!(!rendered.contains(env.get("TONAPI_KEY").unwrap()));
    assert!(!rendered.contains(env.get("TONCENTER_API_KEY").unwrap()));
    assert!(!rendered.contains(env.get("DATABASE_URL").unwrap()));
    assert!(!rendered.contains(env.get("REDIS_URL").unwrap()));
    assert!(!rendered.contains(env.get("L2_ADMIN_TOKEN").unwrap()));
    assert!(!rendered.contains(env.get("L1_COMMIT_SIGNER_TOKEN").unwrap()));
    assert!(!rendered.contains(env.get("L1_COMMIT_SIGNER_ENDPOINT").unwrap()));
}
