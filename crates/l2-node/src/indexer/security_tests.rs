use super::*;
use crate::storage::{DynStorage, InMemoryStorage};
use l2_core::{L2TransactionKind, ReceiptStatus};
use serde_json::json;
use std::collections::VecDeque;
use tokio::sync::Mutex;

const VAULT: &str = "EQvault";
const ATTACKER: &str = "EQattacker";

#[tokio::test]
async fn forged_source_with_valid_body_is_rejected_before_credit() {
    let mut message = deposit_message(7, 10, hash(0x44));
    message.source = Some(ATTACKER.to_owned());

    let error = parse_deposit_message(&message, &config()).expect_err("forged source");

    assert!(matches!(
        error,
        IndexerError::Validation("deposit log source is not vault")
    ));
}

#[test]
fn canonical_deposit_id_binds_source_l1_hash_and_lt() {
    let event_id = hash(0x11);
    let base = canonical_deposit_id(VAULT, hash(0x44), 7, event_id);

    assert_ne!(
        base,
        canonical_deposit_id(ATTACKER, hash(0x44), 7, event_id)
    );
    assert_ne!(base, canonical_deposit_id(VAULT, hash(0x45), 7, event_id));
    assert_ne!(base, canonical_deposit_id(VAULT, hash(0x44), 8, event_id));
    assert_ne!(base, canonical_deposit_id(VAULT, hash(0x44), 7, hash(0x12)));
}

#[tokio::test]
async fn poll_rejects_unexpected_asset_without_cursor_or_credit() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let mut message = deposit_message(7, 10, hash(0x44));
    set_asset_id(&mut message, 2);
    let client = MockTonClient::new(vec![Ok(vec![message])]);
    let indexer = TonDepositIndexer::new(config(), client);

    let error = indexer
        .poll_once(&storage, &sequencer)
        .await
        .expect_err("unexpected asset");

    assert!(matches!(
        error,
        IndexerError::Validation("unexpected deposit asset id")
    ));
    assert!(storage
        .get_l1_cursor(&config().cursor_source())
        .await
        .unwrap()
        .is_none());
    assert!(sequencer.write().await.produce_block(100).is_none());
}

#[tokio::test]
async fn jetton_asset_deposit_credits_only_registered_asset_id() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let mut config = config();
    config.allowed_asset_ids = vec![1, 2];
    let mut message = deposit_message(7, 123, hash(0x44));
    set_asset_id(&mut message, 2);
    let client = MockTonClient::new(vec![Ok(vec![message])]);
    let indexer = TonDepositIndexer::new(config, client);

    let stats = indexer.poll_once(&storage, &sequencer).await.expect("poll");

    assert_eq!(stats.accepted, 1);
    let block = sequencer.write().await.produce_block(100).expect("block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Applied);
    let L2TransactionKind::Deposit {
        asset_id, amount, ..
    } = block.transactions[0].kind
    else {
        panic!("expected deposit tx");
    };
    assert_eq!(asset_id, 2);
    assert_eq!(amount, 123);

    let sequencer = sequencer.read().await;
    let account = sequencer.state.account(hash(0x22)).expect("recipient");
    assert_eq!(account.balance(2), 123);
    assert_eq!(account.balance(1), 0);
    assert_eq!(account.balance(0), 0);
}

#[tokio::test]
async fn valid_prefix_before_malformed_tail_does_not_advance_past_tail() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let valid = deposit_message(7, 10, hash(0x44));
    let mut malformed = deposit_message(8, 10, hash(0x45));
    malformed.message_content = None;
    let client = MockTonClient::new(vec![
        Ok(vec![valid, malformed.clone()]),
        Ok(vec![malformed]),
    ]);
    let indexer = TonDepositIndexer::new(config(), client);

    assert!(indexer.poll_once(&storage, &sequencer).await.is_err());
    let cursor = storage
        .get_l1_cursor(&config().cursor_source())
        .await
        .unwrap()
        .expect("cursor after accepted prefix");
    assert_eq!(cursor.lt, 7);
    assert_eq!(cursor.hash, hash(0x44));

    let block = sequencer.write().await.produce_block(100).expect("block");
    assert_eq!(block.transactions.len(), 1);
    assert_eq!(block.receipts[0].status, ReceiptStatus::Applied);

    assert!(indexer.poll_once(&storage, &sequencer).await.is_err());
    let cursor = storage
        .get_l1_cursor(&config().cursor_source())
        .await
        .unwrap()
        .expect("cursor remains at valid prefix");
    assert_eq!(cursor.lt, 7);
    assert!(sequencer.write().await.produce_block(101).is_none());
}

#[tokio::test]
async fn confirmation_lag_holds_back_only_unconfirmed_tail_without_cursor() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let sequencer = Arc::new(RwLock::new(Sequencer::new(Default::default())));
    let mut config = config();
    config.confirmation_lag_lt = 5;
    let client = MockTonClient::new(vec![Ok(vec![deposit_message(10, 10, hash(0x44))])]);
    let indexer = TonDepositIndexer::new(config.clone(), client);

    let stats = indexer.poll_once(&storage, &sequencer).await.expect("poll");

    assert_eq!(stats.fetched, 1);
    assert_eq!(stats.accepted, 0);
    assert!(storage
        .get_l1_cursor(&config.cursor_source())
        .await
        .unwrap()
        .is_none());
    assert!(sequencer.write().await.produce_block(100).is_none());
}

#[derive(Clone, Default)]
struct MockTonClient {
    responses: Arc<Mutex<VecDeque<Result<Vec<ToncenterMessage>, IndexerError>>>>,
}

impl MockTonClient {
    fn new(responses: Vec<Result<Vec<ToncenterMessage>, IndexerError>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
        }
    }
}

#[async_trait]
impl TonMessageClient for MockTonClient {
    async fn get_deposit_logs(
        &self,
        _request: ToncenterMessagesRequest,
    ) -> Result<Vec<ToncenterMessage>, IndexerError> {
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

fn set_asset_id(message: &mut ToncenterMessage, asset_id: u32) {
    message
        .message_content
        .as_mut()
        .expect("content")
        .decoded
        .as_mut()
        .expect("decoded")["assetId"] = json!(asset_id);
}

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32])
}
