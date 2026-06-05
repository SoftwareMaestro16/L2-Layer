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
    assert!(!config.l1_batch_finalizer_enabled);
    assert_eq!(config.l1_deposit_asset_ids, vec![1]);
    assert_eq!(config.mempool_replay_ttl_secs, 86_400);
    assert_eq!(config.mempool_nonce_lock_ttl_secs, 300);
    assert_eq!(config.mempool_max_global_queue, 10_000);
    assert_eq!(config.mempool_max_account_queue, 64);
    assert_eq!(config.mempool_max_account_nonce_window, 256);
    assert_eq!(config.mempool_max_ip_submissions_per_window, 600);
    assert!(config.mempool_banned_ips.is_empty());
    assert!(config.mempool_banned_accounts.is_empty());
    assert_eq!(config.da_max_payload_bytes, DEFAULT_DA_MAX_PAYLOAD_BYTES);
    assert_eq!(config.da_public_backend, DEFAULT_DA_PUBLIC_BACKEND);
    assert_eq!(
        config.da_public_fs_dir,
        PathBuf::from(DEFAULT_DA_PUBLIC_FS_DIR)
    );
    assert_eq!(config.da_public_base_url, None);
    assert_eq!(
        config.executor_gas_schedule,
        l2_core::GasSchedule::default()
    );
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
fn config_validates_l1_batch_finalizer_settings() {
    let mut env = valid_env();
    env.insert("L1_BATCH_FINALIZER_ENABLED".to_owned(), "true".to_owned());
    assert!(load_from(&env).is_err());

    env.insert("L1_ROLLUP_ROOT_ADDRESS".to_owned(), "EQroot".to_owned());
    env.insert(
        "L1_SEQUENCER_SENDER_ADDRESS".to_owned(),
        "EQsequencer".to_owned(),
    );
    env.insert(
        "L1_FINALIZE_SIGNER_ENDPOINT".to_owned(),
        "not-a-url".to_owned(),
    );
    env.insert(
        "L1_FINALIZE_SIGNER_TOKEN".to_owned(),
        "test-finalize-signer-token".to_owned(),
    );
    assert!(load_from(&env).is_err());

    env.insert(
        "L1_FINALIZE_SIGNER_ENDPOINT".to_owned(),
        "http://127.0.0.1:8800/sign-finalize".to_owned(),
    );
    env.insert("L1_BATCH_FINALIZER_MAX_ATTEMPTS".to_owned(), "0".to_owned());
    assert!(load_from(&env).is_err());

    env.insert("L1_BATCH_FINALIZER_MAX_ATTEMPTS".to_owned(), "8".to_owned());
    let config = load_from(&env).expect("finalizer config");
    assert!(config.l1_batch_finalizer_enabled);
    assert_eq!(
        config.l1_finalize_signer_endpoint.as_deref(),
        Some("http://127.0.0.1:8800/sign-finalize")
    );
}

#[test]
fn config_validates_da_limits() {
    let mut env = valid_env();
    env.insert("DA_MAX_PAYLOAD_BYTES".to_owned(), "4096".to_owned());
    env.insert("DA_PUBLIC_BACKEND".to_owned(), "filesystem".to_owned());
    env.insert("DA_PUBLIC_FS_DIR".to_owned(), "build/da-public".to_owned());
    env.insert(
        "DA_PUBLIC_BASE_URL".to_owned(),
        "https://da.example.test/entropis".to_owned(),
    );
    let config = load_from(&env).expect("da config");
    assert_eq!(config.da_max_payload_bytes, 4096);
    assert_eq!(config.da_public_backend, "filesystem");
    assert_eq!(config.da_public_fs_dir, PathBuf::from("build/da-public"));
    assert_eq!(
        config.da_public_base_url.as_deref(),
        Some("https://da.example.test/entropis")
    );

    let mut env = valid_env();
    env.insert("DA_MAX_PAYLOAD_BYTES".to_owned(), "0".to_owned());
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert(
        "DA_MAX_PAYLOAD_BYTES".to_owned(),
        (129 * 1024 * 1024).to_string(),
    );
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert("DA_PUBLIC_BACKEND".to_owned(), "s3".to_owned());
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert("DA_PUBLIC_BACKEND".to_owned(), "filesystem".to_owned());
    env.insert("DA_PUBLIC_BASE_URL".to_owned(), "file:///tmp/da".to_owned());
    assert!(load_from(&env).is_err());
}

#[test]
fn config_validates_mempool_admission_limits() {
    let mut env = valid_env();
    env.insert("MEMPOOL_MAX_GLOBAL_QUEUE".to_owned(), "10".to_owned());
    env.insert("MEMPOOL_MAX_ACCOUNT_QUEUE".to_owned(), "2".to_owned());
    env.insert(
        "MEMPOOL_MAX_ACCOUNT_NONCE_WINDOW".to_owned(),
        "8".to_owned(),
    );
    env.insert(
        "MEMPOOL_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW".to_owned(),
        "3".to_owned(),
    );
    env.insert(
        "MEMPOOL_MAX_IP_SUBMISSIONS_PER_WINDOW".to_owned(),
        "30".to_owned(),
    );
    env.insert("MEMPOOL_MAX_PAYLOAD_BYTES".to_owned(), "512".to_owned());
    env.insert(
        "MEMPOOL_MAX_TRANSFER_PAYLOAD_BYTES".to_owned(),
        "256".to_owned(),
    );
    env.insert(
        "MEMPOOL_MAX_WITHDRAW_PAYLOAD_BYTES".to_owned(),
        "256".to_owned(),
    );
    env.insert(
        "MEMPOOL_MAX_CALL_PAYLOAD_BYTES".to_owned(),
        "384".to_owned(),
    );
    env.insert(
        "MEMPOOL_MAX_DEPLOY_PAYLOAD_BYTES".to_owned(),
        "512".to_owned(),
    );
    env.insert(
        "MEMPOOL_MAX_CALL_BODY_BOC_BASE64_BYTES".to_owned(),
        "256".to_owned(),
    );
    env.insert("MEMPOOL_MIN_GAS_LIMIT".to_owned(), "10".to_owned());
    env.insert("MEMPOOL_MAX_GAS_LIMIT".to_owned(), "1000".to_owned());
    env.insert("MEMPOOL_MIN_GAS_PRICE".to_owned(), "2".to_owned());
    env.insert("MEMPOOL_MAX_TX_FEE".to_owned(), "10000".to_owned());
    env.insert("MEMPOOL_POP_BATCH_SIZE".to_owned(), "4".to_owned());
    env.insert("MEMPOOL_BANNED_IPS".to_owned(), "127.0.0.2,::1".to_owned());
    env.insert(
        "MEMPOOL_BANNED_ACCOUNTS".to_owned(),
        "8:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
    );
    let config = load_from(&env).expect("mempool config");
    assert_eq!(config.mempool_max_global_queue, 10);
    assert_eq!(config.mempool_max_account_queue, 2);
    assert_eq!(config.mempool_max_account_nonce_window, 8);
    assert_eq!(config.mempool_max_ip_submissions_per_window, 30);
    assert_eq!(config.mempool_pop_batch_size, 4);
    assert_eq!(config.mempool_banned_ips.len(), 2);
    assert_eq!(config.mempool_banned_accounts.len(), 1);

    let mut env = valid_env();
    env.insert("MEMPOOL_MAX_ACCOUNT_QUEUE".to_owned(), "11".to_owned());
    env.insert("MEMPOOL_MAX_GLOBAL_QUEUE".to_owned(), "10".to_owned());
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert("MEMPOOL_REPLAY_TTL_SECS".to_owned(), "0".to_owned());
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert(
        "MEMPOOL_MAX_ACCOUNT_NONCE_WINDOW".to_owned(),
        "0".to_owned(),
    );
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert("MEMPOOL_MIN_GAS_LIMIT".to_owned(), "100".to_owned());
    env.insert("MEMPOOL_MAX_GAS_LIMIT".to_owned(), "10".to_owned());
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert("MEMPOOL_BANNED_IPS".to_owned(), "not-an-ip".to_owned());
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert(
        "MEMPOOL_BANNED_ACCOUNTS".to_owned(),
        "not-an-l2-address".to_owned(),
    );
    assert!(load_from(&env).is_err());
}

#[test]
fn config_validates_executor_gas_schedule() {
    let mut env = valid_env();
    env.insert("EXECUTOR_TRANSFER_GAS".to_owned(), "12".to_owned());
    env.insert("EXECUTOR_WITHDRAW_GAS".to_owned(), "24".to_owned());
    env.insert("EXECUTOR_CALL_CONTRACT_GAS".to_owned(), "60".to_owned());
    env.insert("EXECUTOR_REJECTED_EXECUTION_GAS".to_owned(), "2".to_owned());
    env.insert("EXECUTOR_MIN_GAS_PRICE".to_owned(), "3".to_owned());
    let config = load_from(&env).expect("executor gas config");
    assert_eq!(config.executor_gas_schedule.transfer_gas, 12);
    assert_eq!(config.executor_gas_schedule.withdraw_gas, 24);
    assert_eq!(config.executor_gas_schedule.call_contract_gas, 60);
    assert_eq!(config.executor_gas_schedule.rejected_execution_gas, 2);
    assert_eq!(config.executor_gas_schedule.min_gas_price, 3);

    let mut env = valid_env();
    env.insert("EXECUTOR_GAS_SCHEDULE_VERSION".to_owned(), "2".to_owned());
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert("EXECUTOR_TRANSFER_GAS".to_owned(), "0".to_owned());
    assert!(load_from(&env).is_err());

    let mut env = valid_env();
    env.insert("EXECUTOR_MIN_GAS_PRICE".to_owned(), "0".to_owned());
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

    let mut env = valid_env();
    env.insert("L1_BATCH_FINALIZER_ENABLED".to_owned(), "true".to_owned());
    env.insert("L1_ROLLUP_ROOT_ADDRESS".to_owned(), "EQroot".to_owned());
    env.insert(
        "L1_SEQUENCER_SENDER_ADDRESS".to_owned(),
        "EQsequencer".to_owned(),
    );
    env.insert(
        "L1_FINALIZE_SIGNER_ENDPOINT".to_owned(),
        "http://127.0.0.1:8800/sign-finalize".to_owned(),
    );
    env.insert(
        "L1_FINALIZE_SIGNER_TOKEN".to_owned(),
        "test-finalize-signer-token".to_owned(),
    );
    let config = load_from(&env).expect("finalizer config");
    let debug = format!("{config:?}");
    assert!(!debug.contains(env.get("L1_FINALIZE_SIGNER_TOKEN").unwrap()));
}
