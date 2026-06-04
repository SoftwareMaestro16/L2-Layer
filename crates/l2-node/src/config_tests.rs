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
        (
            "L2_DEV_ADMIN_DEPOSITS_ENABLED".to_owned(),
            "true".to_owned(),
        ),
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
    assert!(config.dev_admin_deposits_enabled);
    assert!(!config.l1_deposit_indexer_enabled);
    assert!(!config.l1_batch_relayer_enabled);
    assert_eq!(config.l1_deposit_asset_ids, vec![1]);
    assert_eq!(config.mempool_replay_ttl_secs, 86_400);
    assert_eq!(config.mempool_nonce_lock_ttl_secs, 300);
    assert_eq!(config.mempool_max_global_queue, 10_000);
    assert_eq!(config.mempool_max_account_queue, 64);
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
fn config_validates_l1_indexer_settings() {
    let mut env = valid_env();
    env.insert("L1_DEPOSIT_INDEXER_ENABLED".to_owned(), "true".to_owned());
    assert!(load_from(&env).is_err());

    env.insert(
        "L1_VAULT_ADDRESS".to_owned(),
        "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c".to_owned(),
    );
    env.insert("L1_DEPOSIT_BATCH_LIMIT".to_owned(), "0".to_owned());
    assert!(load_from(&env).is_err());

    env.insert("L1_DEPOSIT_BATCH_LIMIT".to_owned(), "100".to_owned());
    env.insert("L1_TON_ASSET_ID".to_owned(), "0".to_owned());
    assert!(load_from(&env).is_err());

    env.insert("L1_TON_ASSET_ID".to_owned(), "1".to_owned());
    env.insert("L1_DEPOSIT_ASSET_IDS".to_owned(), "1,2,2".to_owned());
    let config = load_from(&env).expect("indexer config");
    assert!(config.l1_deposit_indexer_enabled);
    assert_eq!(config.l1_deposit_batch_limit, 100);
    assert_eq!(config.l1_deposit_asset_ids, vec![1, 2]);

    let mut env = valid_env();
    env.insert("L1_DEPOSIT_ASSET_IDS".to_owned(), "2".to_owned());
    let config = load_from(&env).expect("auto includes ton asset");
    assert_eq!(config.l1_deposit_asset_ids, vec![1, 2]);

    let mut env = valid_env();
    env.insert("L1_DEPOSIT_ASSET_IDS".to_owned(), "0,1".to_owned());
    assert!(load_from(&env).is_err());
}

#[test]
fn config_validates_l1_batch_relayer_settings() {
    let mut env = valid_env();
    env.insert("L1_BATCH_RELAYER_ENABLED".to_owned(), "true".to_owned());
    assert!(load_from(&env).is_err());

    env.insert("L1_ROLLUP_ROOT_ADDRESS".to_owned(), "EQroot".to_owned());
    env.insert(
        "L1_SEQUENCER_SENDER_ADDRESS".to_owned(),
        "EQsequencer".to_owned(),
    );
    env.insert(
        "L1_COMMIT_SIGNER_ENDPOINT".to_owned(),
        "not-a-url".to_owned(),
    );
    env.insert(
        "L1_COMMIT_SIGNER_TOKEN".to_owned(),
        "test-signer-token".to_owned(),
    );
    assert!(load_from(&env).is_err());

    env.insert(
        "L1_COMMIT_SIGNER_ENDPOINT".to_owned(),
        "http://127.0.0.1:8800/sign-commit".to_owned(),
    );
    env.insert("L1_BATCH_RELAYER_MAX_ATTEMPTS".to_owned(), "0".to_owned());
    assert!(load_from(&env).is_err());

    env.insert("L1_BATCH_RELAYER_MAX_ATTEMPTS".to_owned(), "8".to_owned());
    let config = load_from(&env).expect("relayer config");
    assert!(config.l1_batch_relayer_enabled);
    assert_eq!(config.l1_rollup_root_address.as_deref(), Some("EQroot"));
    assert_eq!(
        config.l1_sequencer_sender_address.as_deref(),
        Some("EQsequencer")
    );
}

#[test]
fn config_validates_mempool_admission_limits() {
    let mut env = valid_env();
    env.insert("MEMPOOL_MAX_GLOBAL_QUEUE".to_owned(), "10".to_owned());
    env.insert("MEMPOOL_MAX_ACCOUNT_QUEUE".to_owned(), "2".to_owned());
    env.insert(
        "MEMPOOL_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW".to_owned(),
        "3".to_owned(),
    );
    env.insert("MEMPOOL_MAX_PAYLOAD_BYTES".to_owned(), "512".to_owned());
    env.insert(
        "MEMPOOL_MAX_CALL_BODY_BOC_BASE64_BYTES".to_owned(),
        "256".to_owned(),
    );
    env.insert("MEMPOOL_MIN_GAS_LIMIT".to_owned(), "10".to_owned());
    env.insert("MEMPOOL_MAX_GAS_LIMIT".to_owned(), "1000".to_owned());
    env.insert("MEMPOOL_MIN_GAS_PRICE".to_owned(), "2".to_owned());
    env.insert("MEMPOOL_MAX_TX_FEE".to_owned(), "10000".to_owned());
    env.insert("MEMPOOL_POP_BATCH_SIZE".to_owned(), "4".to_owned());
    let config = load_from(&env).expect("mempool config");
    assert_eq!(config.mempool_max_global_queue, 10);
    assert_eq!(config.mempool_max_account_queue, 2);
    assert_eq!(config.mempool_pop_batch_size, 4);

    let mut env = valid_env();
    env.insert("MEMPOOL_MAX_ACCOUNT_QUEUE".to_owned(), "11".to_owned());
    env.insert("MEMPOOL_MAX_GLOBAL_QUEUE".to_owned(), "10".to_owned());
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert("MEMPOOL_REPLAY_TTL_SECS".to_owned(), "0".to_owned());
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert("MEMPOOL_MIN_GAS_LIMIT".to_owned(), "100".to_owned());
    env.insert("MEMPOOL_MAX_GAS_LIMIT".to_owned(), "10".to_owned());
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

    let mut env = valid_env();
    env.insert("L1_BATCH_RELAYER_ENABLED".to_owned(), "true".to_owned());
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
    let config = load_from(&env).expect("relayer config");
    let debug = format!("{config:?}");
    assert!(!debug.contains(env.get("L1_COMMIT_SIGNER_TOKEN").unwrap()));
}
