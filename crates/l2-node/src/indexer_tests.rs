use super::*;
use crate::storage::{DynStorage, InMemoryStorage};
use l2_core::{L2TransactionKind, ReceiptStatus};
use serde_json::json;
use std::collections::VecDeque;
use tokio::sync::Mutex;

const VAULT: &str = "EQvault";

#[tokio::test]
async fn parses_valid_deposit_recorded_event() {
    let deposit =
        parse_deposit_message(&deposit_message(7, 10, hash(0x44)), &config()).expect("deposit");

    assert_eq!(deposit.asset_id, 1);
    assert_eq!(deposit.amount, 10);
    assert_eq!(deposit.l1_lt, 7);
    assert_eq!(deposit.l1_tx_hash, hash(0x44));
    assert_eq!(deposit.recipient, hash(0x22));
}

#[tokio::test]
async fn parses_valid_jetton_deposit_recorded_event() {
    let mut config = config();
    config.allowed_asset_ids = vec![1, 2];
    let mut message = deposit_message(8, 123, hash(0x45));
    message
        .message_content
        .as_mut()
        .unwrap()
        .decoded
        .as_mut()
        .unwrap()["assetId"] = json!(2);

    let deposit = parse_deposit_message(&message, &config).expect("jetton deposit");

    assert_eq!(deposit.asset_id, 2);
    assert_eq!(deposit.amount, 123);
    assert_eq!(deposit.l1_lt, 8);
    assert_eq!(deposit.l1_tx_hash, hash(0x45));
}

#[tokio::test]
async fn parses_sanitized_toncenter_jetton_deposit_fixture() {
    let mut config = config();
    config.allowed_asset_ids = vec![1, 7];
    let message = ToncenterMessage {
        hash: None,
        hash_norm: Some(hash(0x46).to_hex()),
        source: Some(VAULT.to_owned()),
        destination: Some("null".to_owned()),
        opcode: Some(json!(0x4c324407)),
        created_lt: Some(json!(9)),
        message_content: Some(ToncenterMessageContent {
            body: Some("sanitized-boc".to_owned()),
            decoded: Some(json!({
                "query_id": "7001",
                "deposit_id": hash(0x31).to_hex(),
                "asset_id": 7,
                "amount": "123000000",
                "l2_recipient": hash(0x32).to_hex(),
            })),
        }),
    };

    let deposit = parse_deposit_message(&message, &config).expect("jetton deposit fixture");

    assert_eq!(
        deposit.deposit_id,
        canonical_deposit_id(VAULT, hash(0x46), 9, hash(0x31))
    );
    assert_eq!(deposit.asset_id, 7);
    assert_eq!(deposit.amount, 123000000);
    assert_eq!(deposit.recipient, hash(0x32));
    assert_eq!(deposit.l1_lt, 9);
    assert_eq!(deposit.l1_tx_hash, hash(0x46));
}

#[tokio::test]
async fn rejects_forged_or_malformed_events() {
    let mut forged = deposit_message(7, 10, hash(0x44));
    forged.source = Some("EQattacker".to_owned());
    assert!(parse_deposit_message(&forged, &config()).is_err());

    let mut malformed = deposit_message(7, 10, hash(0x44));
    malformed.message_content = None;
    assert!(parse_deposit_message(&malformed, &config()).is_err());

    let mut not_log = deposit_message(7, 10, hash(0x44));
    not_log.destination = Some("EQrecipient".to_owned());
    assert!(parse_deposit_message(&not_log, &config()).is_err());

    let mut wrong_opcode = deposit_message(7, 10, hash(0x44));
    wrong_opcode.opcode = Some(json!("0xdeadbeef"));
    assert!(parse_deposit_message(&wrong_opcode, &config()).is_err());

    let zero_lt = deposit_message(0, 10, hash(0x44));
    assert!(parse_deposit_message(&zero_lt, &config()).is_err());

    let zero_hash = deposit_message(7, 10, Hash32::ZERO);
    assert!(parse_deposit_message(&zero_hash, &config()).is_err());

    let mut wrong_asset = deposit_message(7, 10, hash(0x44));
    wrong_asset
        .message_content
        .as_mut()
        .unwrap()
        .decoded
        .as_mut()
        .unwrap()["assetId"] = json!(2);
    assert!(parse_deposit_message(&wrong_asset, &config()).is_err());

    let mut zero_amount = deposit_message(7, 10, hash(0x44));
    zero_amount
        .message_content
        .as_mut()
        .unwrap()
        .decoded
        .as_mut()
        .unwrap()["amount"] = json!("0");
    assert!(parse_deposit_message(&zero_amount, &config()).is_err());

    let mut zero_recipient = deposit_message(7, 10, hash(0x44));
    zero_recipient
        .message_content
        .as_mut()
        .unwrap()
        .decoded
        .as_mut()
        .unwrap()["l2Recipient"] = json!(Hash32::ZERO.to_hex());
    assert!(parse_deposit_message(&zero_recipient, &config()).is_err());
}

#[tokio::test]
async fn poll_advances_cursor_and_ingests_deposit() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let client = MockTonClient::new(vec![Ok(vec![deposit_message(7, 10, hash(0x44))])]);
    let indexer = TonDepositIndexer::new(config(), client.clone());

    let stats = indexer.poll_once(&storage, &sequencer).await.expect("poll");

    assert_eq!(stats.accepted, 1);
    assert_eq!(client.requests().await[0].start_lt, 1);
    let cursor = storage
        .get_l1_cursor(&config().cursor_source())
        .await
        .unwrap()
        .expect("cursor");
    assert_eq!(cursor.lt, 7);
    assert_eq!(cursor.hash, hash(0x44));

    let mut sequencer = sequencer.write().await;
    let block = sequencer.produce_block(100).expect("deposit block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Applied);
    assert_eq!(sequencer.state.account(hash(0x22)).unwrap().balance(1), 10);
}

#[tokio::test]
async fn duplicate_deposit_is_not_ingested_twice() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let message = deposit_message(7, 10, hash(0x44));
    let client = MockTonClient::new(vec![Ok(vec![message.clone()]), Ok(vec![message])]);
    let indexer = TonDepositIndexer::new(config(), client);

    let first = indexer
        .poll_once(&storage, &sequencer)
        .await
        .expect("first");
    assert_eq!(first.accepted, 1);
    sequencer.write().await.produce_block(100);

    let second = indexer
        .poll_once(&storage, &sequencer)
        .await
        .expect("second");
    assert_eq!(second.accepted, 0);
    assert_eq!(second.duplicates, 1);
    assert!(sequencer.write().await.produce_block(101).is_none());
}

#[tokio::test]
async fn repeated_contract_deposit_id_with_new_l1_identity_is_credited() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let first = deposit_message(7, 10, hash(0x44));
    let second = deposit_message(8, 10, hash(0x45));
    let client = MockTonClient::new(vec![Ok(vec![first, second])]);
    let indexer = TonDepositIndexer::new(config(), client);

    let stats = indexer.poll_once(&storage, &sequencer).await.expect("poll");
    assert_eq!(stats.accepted, 2);

    let block = sequencer.write().await.produce_block(100).expect("block");
    let first_id = match &block.transactions[0].kind {
        L2TransactionKind::Deposit { deposit_id, .. } => *deposit_id,
        _ => panic!("expected deposit tx"),
    };
    let second_id = match &block.transactions[1].kind {
        L2TransactionKind::Deposit { deposit_id, .. } => *deposit_id,
        _ => panic!("expected deposit tx"),
    };

    assert_ne!(first_id, second_id);
    assert_eq!(
        sequencer
            .read()
            .await
            .state
            .account(hash(0x22))
            .unwrap()
            .balance(1),
        20
    );
}

#[tokio::test]
async fn duplicate_jetton_deposit_is_not_ingested_twice() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let mut config = config();
    config.allowed_asset_ids = vec![1, 2];
    let mut message = deposit_message(7, 10, hash(0x44));
    message
        .message_content
        .as_mut()
        .unwrap()
        .decoded
        .as_mut()
        .unwrap()["assetId"] = json!(2);
    let client = MockTonClient::new(vec![Ok(vec![message.clone()]), Ok(vec![message])]);
    let indexer = TonDepositIndexer::new(config, client);

    let first = indexer
        .poll_once(&storage, &sequencer)
        .await
        .expect("first");
    assert_eq!(first.accepted, 1);
    sequencer.write().await.produce_block(100);

    let second = indexer
        .poll_once(&storage, &sequencer)
        .await
        .expect("second");
    assert_eq!(second.accepted, 0);
    assert_eq!(second.duplicates, 1);
    assert!(sequencer.write().await.produce_block(101).is_none());
}

#[tokio::test]
async fn same_l1_cursor_with_different_deposit_id_is_duplicate() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let message = deposit_message(7, 10, hash(0x44));
    let mut replay = deposit_message(7, 10, hash(0x44));
    replay
        .message_content
        .as_mut()
        .unwrap()
        .decoded
        .as_mut()
        .unwrap()["depositId"] = json!(hash(0x33).to_hex());
    let client = MockTonClient::new(vec![Ok(vec![message]), Ok(vec![replay])]);
    let indexer = TonDepositIndexer::new(config(), client);

    let first = indexer
        .poll_once(&storage, &sequencer)
        .await
        .expect("first");
    assert_eq!(first.accepted, 1);
    sequencer.write().await.produce_block(100);

    let second = indexer
        .poll_once(&storage, &sequencer)
        .await
        .expect("second");
    assert_eq!(second.accepted, 0);
    assert_eq!(second.duplicates, 1);
    assert!(sequencer.write().await.produce_block(101).is_none());
}

#[tokio::test]
async fn malformed_response_does_not_advance_cursor_and_can_retry() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let mut malformed = deposit_message(7, 10, hash(0x44));
    malformed.message_content = None;
    let client = MockTonClient::new(vec![
        Ok(vec![malformed]),
        Ok(vec![deposit_message(7, 10, hash(0x44))]),
    ]);
    let indexer = TonDepositIndexer::new(config(), client);

    assert!(indexer.poll_once(&storage, &sequencer).await.is_err());
    assert!(storage
        .get_l1_cursor(&config().cursor_source())
        .await
        .unwrap()
        .is_none());

    let retry = indexer
        .poll_once(&storage, &sequencer)
        .await
        .expect("retry");
    assert_eq!(retry.accepted, 1);
}

#[tokio::test]
async fn cursor_start_lt_uses_previous_cursor_plus_one() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    storage
        .set_l1_cursor(
            &config().cursor_source(),
            L1Cursor {
                lt: 41,
                hash: hash(0x41),
            },
        )
        .await
        .unwrap();
    let client = MockTonClient::new(vec![Ok(vec![])]);
    let indexer = TonDepositIndexer::new(config(), client.clone());

    indexer.poll_once(&storage, &sequencer).await.expect("poll");

    assert_eq!(client.requests().await[0].start_lt, 42);
}

#[tokio::test]
async fn confirmation_lag_holds_back_unconfirmed_tail() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let mut config = config();
    config.confirmation_lag_lt = 5;
    let client = MockTonClient::new(vec![Ok(vec![
        deposit_message(10, 10, hash(0x10)),
        deposit_message(13, 20, hash(0x13)),
        deposit_message(16, 30, hash(0x16)),
    ])]);
    let indexer = TonDepositIndexer::new(config.clone(), client);

    let stats = indexer.poll_once(&storage, &sequencer).await.expect("poll");

    assert_eq!(stats.accepted, 1);
    let cursor = storage
        .get_l1_cursor(&config.cursor_source())
        .await
        .unwrap()
        .expect("cursor");
    assert_eq!(cursor.lt, 10);
}

#[derive(Clone, Default)]
struct MockTonClient {
    responses: Arc<Mutex<VecDeque<Result<Vec<ToncenterMessage>, IndexerError>>>>,
    requests: Arc<Mutex<Vec<ToncenterMessagesRequest>>>,
}

impl MockTonClient {
    fn new(responses: Vec<Result<Vec<ToncenterMessage>, IndexerError>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
            requests: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn requests(&self) -> Vec<ToncenterMessagesRequest> {
        self.requests.lock().await.clone()
    }
}

#[async_trait]
impl TonMessageClient for MockTonClient {
    async fn get_deposit_logs(
        &self,
        request: ToncenterMessagesRequest,
    ) -> Result<Vec<ToncenterMessage>, IndexerError> {
        self.requests.lock().await.push(request);
        self.responses
            .lock()
            .await
            .pop_front()
            .expect("mock response")
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

fn deposit_message(lt: u64, amount: u128, message_hash: Hash32) -> ToncenterMessage {
    ToncenterMessage {
        hash: Some(message_hash.to_hex()),
        hash_norm: Some(message_hash.to_hex()),
        source: Some(VAULT.to_owned()),
        destination: None,
        opcode: Some(json!("0x4c324407")),
        created_lt: Some(json!(lt.to_string())),
        message_content: Some(ToncenterMessageContent {
            body: Some("mock-body".to_owned()),
            decoded: Some(json!({
                "queryId": "77",
                "depositId": hash(0x11).to_hex(),
                "assetId": 1,
                "amount": amount.to_string(),
                "l2Recipient": hash(0x22).to_hex(),
            })),
        }),
    }
}

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32])
}
