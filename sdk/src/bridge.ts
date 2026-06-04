import { beginCell, type Cell } from "@ton/core";
import * as RollupRootGenerated from "./generated/RollupRoot.gen.js";
import type {
  ClaimWithdrawalTonConnectMessageParams,
  DepositTonMessageParams,
  Hash32,
  TonConnectMessage,
  WithdrawalMerkleProof,
  WithdrawalProofLeaf,
  WithdrawalProofResponse,
} from "./types.js";
import {
  hashToUint256,
  normalizeHash32,
  parseTonAddress,
  toDecimalString,
  toPositiveUint,
  toUint,
} from "./validation.js";

const WITHDRAWAL_PROOF_CHUNK_MAX = 3;

export function tonDepositForwardPayload(l2Recipient: Hash32) {
  const recipient = BigInt(`0x${normalizeHash32(l2Recipient)}`);
  return beginCell().storeUint(recipient, 256).endCell();
}

export function jettonDepositForwardPayload(l2Recipient: Hash32) {
  return tonDepositForwardPayload(l2Recipient);
}

export function encodeDepositTonBody(
  queryId: bigint | number | string,
  amount: bigint | number | string,
  l2Recipient: Hash32,
) {
  return beginCell()
    .storeUint(0x4c324405, 32)
    .storeUint(toUint(queryId, "queryId", 64), 64)
    .storeCoins(toPositiveUint(amount, "amount", 120))
    .storeUint(BigInt(`0x${normalizeHash32(l2Recipient)}`), 256)
    .endCell();
}

export function depositTonTonConnectMessage(params: DepositTonMessageParams): TonConnectMessage {
  parseTonAddress(params.vaultAddress);
  const amount = toPositiveUint(params.amount, "amount", 120);
  const body = encodeDepositTonBody(params.queryId, amount, params.l2Recipient);
  return {
    address: params.vaultAddress,
    amount: toDecimalString(amount),
    payload: body.toBoc().toString("base64"),
  };
}

export function releaseAuthorizedCell(leaf: WithdrawalProofLeaf) {
  normalizeHash32(leaf.l2_sender);
  return RollupRootGenerated.ReleaseAuthorized.toCell(
    RollupRootGenerated.ReleaseAuthorized.create({
      withdrawalId: hashToUint256(leaf.withdrawal_id, "withdrawal_id"),
      assetId: toUint(leaf.asset_id, "asset_id", 32),
      recipient: parseTonAddress(leaf.l1_recipient),
      amount: toUint(leaf.amount, "amount", 120),
    }),
  );
}

export function withdrawalMerkleProofCell(proof: WithdrawalMerkleProof) {
  const siblings = proof.siblings.map((sibling, index) =>
    hashToUint256(sibling, `siblings[${index}]`),
  );
  if (siblings.length >= 1 << 16) {
    throw new Error("withdrawal proof has too many siblings");
  }

  const groups: bigint[][] = [];
  for (let offset = 0; offset < siblings.length; offset += WITHDRAWAL_PROOF_CHUNK_MAX) {
    groups.push(siblings.slice(offset, offset + WITHDRAWAL_PROOF_CHUNK_MAX));
  }

  let next = null;
  for (let i = groups.length - 1; i >= 0; i -= 1) {
    next = withdrawalProofChunkCell(groups[i], next);
  }

  const builder = beginCell()
    .storeUint(toUint(proof.leaf_index, "leaf_index", 64), 64)
    .storeUint(siblings.length, 16);
  if (next) {
    builder.storeBit(true).storeRef(next);
  } else {
    builder.storeBit(false);
  }
  return builder.endCell();
}

export function buildClaimWithdrawalBody(proof: WithdrawalProofResponse) {
  normalizeHash32(proof.withdrawal_root);
  const blockHeight = toUint(proof.block_height, "block_height", 64);
  const batchNo = blockHeight + 1n;
  if (batchNo >= (1n << 64n)) {
    throw new Error("batchNo exceeds uint64");
  }
  return RollupRootGenerated.RollupRoot.createCellOfClaimWithdrawal({
    batchNo,
    withdrawalId: hashToUint256(proof.leaf.withdrawal_id, "withdrawal_id"),
    withdrawalLeaf: releaseAuthorizedCell(proof.leaf),
    merkleProof: withdrawalMerkleProofCell(proof.proof),
  });
}

export function claimWithdrawalTonConnectMessage(
  params: ClaimWithdrawalTonConnectMessageParams,
): TonConnectMessage {
  const body = buildClaimWithdrawalBody(params.proof);
  parseTonAddress(params.rollupRootAddress);
  return {
    address: params.rollupRootAddress,
    amount: toDecimalString(toPositiveUint(params.amount, "amount", 120)),
    payload: body.toBoc().toString("base64"),
  };
}

function withdrawalProofChunkCell(siblings: bigint[], next: Cell | null) {
  const padded = [0n, 0n, 0n];
  siblings.forEach((sibling, index) => {
    padded[index] = sibling;
  });

  const builder = beginCell()
    .storeUint(siblings.length, 8)
    .storeUint(padded[0], 256)
    .storeUint(padded[1], 256)
    .storeUint(padded[2], 256);
  if (next) {
    builder.storeBit(true).storeRef(next);
  } else {
    builder.storeBit(false);
  }
  return builder.endCell();
}
