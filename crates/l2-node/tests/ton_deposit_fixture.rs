use l2_core::Hash32;
use l2_node::indexer::{parse_deposit_message, DepositIndexerConfig, ToncenterMessagesResponse};

const VAULT: &str = "EQvault";
const DEPOSIT_OPCODE: &str = "0x4c324407";

#[test]
fn parses_sanitized_toncenter_v3_deposit_fixture() {
    let response: ToncenterMessagesResponse = serde_json::from_str(include_str!(
        "../fixtures/toncenter_v3_deposit_recorded.json"
    ))
    .expect("fixture json");

    assert_eq!(response.messages.len(), 1);
    let deposit = parse_deposit_message(&response.messages[0], &config()).expect("deposit");

    assert_eq!(deposit.asset_id, 1);
    assert_eq!(deposit.amount, 250_000_000);
    assert_eq!(deposit.l1_lt, 70_000_000_000_001);
    assert_eq!(
        deposit.deposit_id,
        Hash32::from_hex("1111111111111111111111111111111111111111111111111111111111111111")
            .unwrap()
    );
    assert_eq!(
        deposit.recipient,
        Hash32::from_hex("2222222222222222222222222222222222222222222222222222222222222222")
            .unwrap()
    );
}

#[tokio::test]
#[ignore = "requires ENTROPIS_LIVE_TON_DEPOSIT=1 and testnet Toncenter env"]
async fn live_toncenter_deposit_indexer_smoke_requires_env() {
    if std::env::var("ENTROPIS_LIVE_TON_DEPOSIT").ok().as_deref() != Some("1") {
        return;
    }
    let vault_address = required_env("L1_VAULT_ADDRESS");
    let api_key = required_env("TONCENTER_API_KEY");
    let base_url = std::env::var("TONCENTER_V3_BASE_URL")
        .unwrap_or_else(|_| "https://testnet.toncenter.com/api/v3".to_owned());
    assert!(
        base_url.contains("testnet"),
        "live deposit smoke refuses non-testnet Toncenter endpoints"
    );
    let start_lt = std::env::var("ENTROPIS_LIVE_TON_DEPOSIT_START_LT")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1);
    let start_lt = start_lt.to_string();

    let response = reqwest::Client::new()
        .get(format!("{}/messages", base_url.trim_end_matches('/')))
        .header("X-API-Key", api_key)
        .query(&[
            ("source", vault_address.as_str()),
            ("destination", "null"),
            ("opcode", DEPOSIT_OPCODE),
            ("start_lt", start_lt.as_str()),
            ("limit", "10"),
            ("sort", "asc"),
        ])
        .send()
        .await
        .expect("toncenter request")
        .error_for_status()
        .expect("toncenter status")
        .json::<ToncenterMessagesResponse>()
        .await
        .expect("toncenter json");

    let config = DepositIndexerConfig {
        vault_address,
        allowed_asset_ids: vec![1],
        batch_limit: 10,
        confirmation_lag_lt: 0,
    };
    for message in &response.messages {
        parse_deposit_message(message, &config).expect("deposit event");
    }
}

fn config() -> DepositIndexerConfig {
    DepositIndexerConfig {
        vault_address: VAULT.to_owned(),
        allowed_asset_ids: vec![1],
        batch_limit: 100,
        confirmation_lag_lt: 0,
    }
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
}
