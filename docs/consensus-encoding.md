# Entropis Consensus Encoding v1

Consensus-critical hashes must never depend on JSON field order, serializer behavior,
or API presentation details. Entropis v1 uses explicit binary bytes for L2 transaction
hashes, receipt leaves, withdrawal leaves, account leaves, block headers, and batch
data commitments.

## Framing

Every top-level encoded object starts with:

```text
magic:   4 bytes  ASCII "EL2C"
version: 1 byte   0x01
type:    1 byte
```

Type tags:

```text
0x01 unsigned transaction
0x02 receipt
0x03 withdrawal leaf
0x04 account leaf
0x05 block header
0x06 batch data
0x07 signed transaction for DA payloads
```

Primitive layout:

```text
uint8       1 byte
uint32      4 bytes, big-endian
uint64      8 bytes, big-endian
uint128     16 bytes, big-endian
Hash32      32 raw bytes
string      uint32 byte length + UTF-8 bytes
bytes       uint32 byte length + raw bytes
optional<T> uint8 0x00 for none, 0x01 + T for some
vector<T>   uint32 item count + items
```

## Objects

Unsigned transaction:

```text
chain_id:string
from:optional<Hash32>
nonce:uint64
gas_limit:uint64
max_gas_price:uint128
kind:uint8 + kind fields
```

Transaction kind tags:

```text
0x01 Deposit(deposit_id:Hash32, asset_id:uint32, recipient:Hash32, amount:uint128)
0x02 Transfer(to:Hash32, asset_id:uint32, amount:uint128)
0x03 Withdraw(asset_id:uint32, amount:uint128, l1_recipient:string)
0x04 CallContract(contract:Hash32, body_boc_base64:string)
```

Receipt:

```text
tx_hash:Hash32
status:uint8        # 0x01 applied, 0x02 rejected
gas_charged:uint128 # gas coin base units charged by the executor
reason:optional<string>
withdrawal_id:optional<Hash32>
```

Withdrawal leaf:

```text
withdrawal_id:Hash32
asset_id:uint32
amount:uint128
l2_sender:Hash32
l1_recipient:string
```

L1 withdrawal claim root:

```text
leaf_hash = repr_hash(ReleaseAuthorized.toCell())
node_hash = repr_hash(beginCell()
    .storeUint(left_hash, 256)
    .storeUint(right_hash, 256)
    .endCell())
```

`ReleaseAuthorized` is the Tolk message cell with opcode `0x4c325206`,
`withdrawal_id:uint256`, `asset_id:uint32`, `recipient:address`, and
`amount:coins`. The binary withdrawal leaf above remains the API/DA object, but
`block_header.withdrawal_root` is the L1-compatible tree root so
`RollupRoot.tolk` can verify claims on TVM without reimplementing the Rust/SDK
binary encoder.

Account leaf:

```text
account_id:Hash32
nonce:uint64
balances:vector<(asset_id:uint32, balance:uint128)> sorted by asset_id
code_hash:Hash32
data_hash:Hash32
storage_root:Hash32
last_lt:uint64
```

Block header:

```text
height:uint64
prev_block_hash:Hash32
prev_state_root:Hash32
state_root:Hash32
tx_root:Hash32
receipt_root:Hash32
withdrawal_root:Hash32
data_hash:Hash32
timestamp:uint64
```

Batch data:

```text
signed_transactions:vector<bytes(encode_signed_transaction)>
receipts:vector<bytes(encode_receipt)>
```

Signed transaction is used only for batch DA commitments and includes the unsigned
transaction body plus `public_key:optional<string>` and `signature:optional<string>`.
The transaction hash and signature payload use only unsigned transaction bytes.
The raw `BatchData` bytes are stored in the DA backend and `data_hash` is
`hash_domain("l2.batch.data.v1", [BatchData bytes])`.

## Hash Domains

All hashes use SHA-256 through `hash_domain(domain, parts)`, where each domain and
part is length-prefixed with `uint64` big-endian.

```text
l2.tx.v1
l2.receipt.leaf.v1
l2.withdrawal.id.v1
l2.withdrawal.leaf.v1
l2.state.account.v1
l2.block.header.v1
l2.batch.data.v1
l2.merkle.node.v1
l2.account.ed25519.v1
```

JSON remains valid for APIs and Postgres presentation storage, but it is not valid
input to consensus roots.

## Golden Vectors

Fixture transaction:

```text
chain_id       entropis-testnet
from           aa..aa
nonce          7
gas_limit      500
max_gas_price  42
kind           Transfer(to=bb..bb, asset_id=0, amount=1000)
```

Expected hashes:

```text
tx_hash          c1a6de1d5b776bdd51ab0fcba6bf4ccb62fd3e317b1a3b485cb7f470d9f3a8ac
receipt_leaf     536c7264a2bc9e0659287068183431b452c614df614bc82f0f25d37b001b8d43
withdrawal_leaf  00164447b3c4fb77bf5a9c2bf179782ef7cc6074ce3057ee6d68feb9d6f5c75e
block_header     9ee765a283d11084ffb5f0819afbf866f70a3e44ca981048c5705f7dbb1417ba
account_leaf     191eda257e6182c35676db70e20e54180e2a7f9eec6cddd4ae5c72a2882f97e9
```

Withdrawal proof vector:

```text
recipient        EQDk2VTvn04SUKJrW7rXahzdF8_Qi6utb0wj43InCu9vdjrR
leaf_index       1
withdrawal_id    bd99c87fa8471211c1fab534ab56b4b5f4d662ecc037f305951eef358d17fad1
withdrawal_root  d5e8e681563ae874899124c32b8bb43072a4d95e0b05b2bf9ddda9ce9d5b62cf
sibling_0        c0f52e7163104fbc3d88592927dd407bfb52f59366bb9ab2eaa354984bf5341e
sibling_1        f93417c921216f9c718722963393bf14ec8183afc14559ddf07b302cabb297ac
claim_boc_base64 te6cckEBBAEA7wACWEwyVwQAAAAAAAAACr2ZyH+oRxIRwfq1NKtWtLX01mLswDfzBZUe7zWNF/rRAQIAlUwyUga9mch/qEcSEcH6tTSrVrS19NZi7MA38wWVHu81jRf60QAAAAGAHJsqnfPpwkoUTWt3Wu1Dm6L5+hF1da3phHxuROFd7e7DkQEVAAAAAAAAAAEAAsADAMMCwPUucWMQT7w9iFkpJ91Ae/tS9ZNmu5qy6qNUmEv1NB75NBfJISFvnHGHIpYzk78U7IGDr8FFWd3wezAsq7KXrAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQJY/TtE=
```

## References

- TON cells: https://docs.ton.org/blockchain-basics/primitives/serialization/cells
- TON bag of cells: https://docs.ton.org/foundations/serialization/boc
- Tolk cells, slices, builders: https://docs.ton.org/blockchain-basics/tolk/types/cells
