import {
  createEnWalletMnemonic,
  enwalletKeyPairFromMnemonic,
  enwalletV5InitialState,
  l2RawAddress,
  l2UserFriendlyAddress,
  signEnWalletV5InitTransaction,
} from "../dist/index.js";

const recoveryWords = process.env.ENTROPIS_MNEMONIC?.trim().split(/\s+/)
  ?? await createEnWalletMnemonic();
const keyPair = await enwalletKeyPairFromMnemonic(recoveryWords);
const wallet = enwalletV5InitialState({ publicKey: keyPair.publicKey });

console.log("Owner raw:", l2RawAddress(wallet.owner_account_id));
console.log("Owner friendly:", l2UserFriendlyAddress(wallet.owner_account_id));
console.log("EnWallet raw:", l2RawAddress(wallet.wallet_account_id));
console.log("EnWallet friendly:", l2UserFriendlyAddress(wallet.wallet_account_id));
console.log("Interface:", wallet.interface_label);
console.log("Code hash:", wallet.code_hash);
console.log("Data hash:", wallet.data_hash);

const initTx = signEnWalletV5InitTransaction({
  chainId: process.env.ENTROPIS_CHAIN_ID ?? "entropis-testnet",
  from: wallet.owner_account_id,
  nonce: Number(process.env.ENTROPIS_OWNER_NONCE ?? 0),
  gasLimit: Number(process.env.ENTROPIS_WALLET_INIT_GAS_LIMIT ?? 50),
  maxGasPrice: process.env.ENTROPIS_MAX_GAS_PRICE ?? "1",
  keyPair,
});

console.log("Init transaction:", JSON.stringify(initTx, null, 2));
