#!/usr/bin/env node

import { depositTonTonConnectMessage } from "../../sdk/dist/index.js";

const message = depositTonTonConnectMessage({
  vaultAddress: required("L1_VAULT_ADDRESS"),
  queryId: optional("DEPOSIT_QUERY_ID") ?? Date.now().toString(),
  amount: required("DEPOSIT_AMOUNT_NANOTON"),
  l2Recipient: required("L2_RECIPIENT"),
});
const msgValue = optional("DEPOSIT_MSG_VALUE_NANOTON") ?? message.amount;
if (!/^[0-9]+$/.test(msgValue) || BigInt(msgValue) < BigInt(message.amount)) {
  throw new Error("DEPOSIT_MSG_VALUE_NANOTON must be >= DEPOSIT_AMOUNT_NANOTON");
}

console.log(
  JSON.stringify(
    {
      network: "-3",
      tonConnectRequest: {
        validUntil: Math.floor(Date.now() / 1000) + 600,
        network: "-3",
        messages: [
          {
            address: message.address,
            amount: msgValue,
            payload: message.payload,
          },
        ],
      },
      rawMessage: {
        to: message.address,
        value: msgValue,
        bodyBocBase64: message.payload,
      },
    },
    null,
    2,
  ),
);

function required(name) {
  const value = optional(name);
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

function optional(name) {
  return process.env[name]?.trim() || undefined;
}
