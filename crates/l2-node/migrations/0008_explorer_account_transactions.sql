CREATE INDEX IF NOT EXISTS l2_transactions_order_idx
    ON l2_transactions(block_height DESC, tx_index DESC);

CREATE INDEX IF NOT EXISTS l2_transactions_from_idx
    ON l2_transactions((tx_json ->> 'from'));

CREATE INDEX IF NOT EXISTS l2_transactions_deposit_recipient_idx
    ON l2_transactions((tx_json #>> '{kind,Deposit,recipient}'));

CREATE INDEX IF NOT EXISTS l2_transactions_transfer_to_idx
    ON l2_transactions((tx_json #>> '{kind,Transfer,to}'));

CREATE INDEX IF NOT EXISTS l2_transactions_deploy_contract_idx
    ON l2_transactions((tx_json #>> '{kind,DeployContract,contract}'));

CREATE INDEX IF NOT EXISTS l2_transactions_call_contract_idx
    ON l2_transactions((tx_json #>> '{kind,CallContract,contract}'));
