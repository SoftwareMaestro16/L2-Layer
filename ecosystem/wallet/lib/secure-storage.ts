"use client";

const DB_NAME = "entropis-enwallet-vault";
const STORE_NAME = "vault";
const RECORD_ID = "primary";
const LEGACY_LOCAL_STORAGE_KEY = "enwallet.entropis-testnet.mnemonic.v1";
const KDF_ITERATIONS = 210_000;

type VaultRecord = {
  id: typeof RECORD_ID;
  version: 1;
  kdf: "PBKDF2-SHA256";
  iterations: number;
  salt: string;
  iv: string;
  ciphertext: string;
  createdAt: string;
};

export type StoredWalletStatus = {
  encrypted: boolean;
  legacyPlaintext: boolean;
};

export async function storedWalletStatus(): Promise<StoredWalletStatus> {
  clearLegacyPlaintextSeed();
  return {
    encrypted: Boolean(await readVaultRecord()),
    legacyPlaintext: false
  };
}

export async function saveEncryptedSeed(seedPhrase: string, password: string): Promise<void> {
  assertPassword(password);
  const salt = randomBytes(16);
  const iv = randomBytes(12);
  const key = await deriveVaultKey(password, salt);
  const ciphertext = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: toArrayBuffer(iv) },
    key,
    new TextEncoder().encode(seedPhrase)
  );
  await writeVaultRecord({
    id: RECORD_ID,
    version: 1,
    kdf: "PBKDF2-SHA256",
    iterations: KDF_ITERATIONS,
    salt: bytesToBase64(salt),
    iv: bytesToBase64(iv),
    ciphertext: bytesToBase64(new Uint8Array(ciphertext)),
    createdAt: new Date().toISOString()
  });
  clearLegacyPlaintextSeed();
}

export async function loadEncryptedSeed(password: string): Promise<string> {
  assertPassword(password);
  const record = await readVaultRecord();
  if (!record) {
    throw new Error("No encrypted EnWallet vault found.");
  }
  try {
    const key = await deriveVaultKey(password, base64ToBytes(record.salt));
    const plaintext = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: toArrayBuffer(base64ToBytes(record.iv)) },
      key,
      toArrayBuffer(base64ToBytes(record.ciphertext))
    );
    return new TextDecoder().decode(plaintext);
  } catch (error) {
    throw new Error("Incorrect password or corrupted wallet vault.", { cause: error });
  }
}

export async function deleteEncryptedSeed(): Promise<void> {
  await withStore("readwrite", (store) => store.delete(RECORD_ID));
  clearLegacyPlaintextSeed();
}

export function clearLegacyPlaintextSeed(): void {
  if (typeof localStorage !== "undefined") {
    localStorage.removeItem(LEGACY_LOCAL_STORAGE_KEY);
  }
}

function assertPassword(password: string) {
  if (password.length < 8) {
    throw new Error("Password must be at least 8 characters.");
  }
}

async function deriveVaultKey(password: string, salt: Uint8Array): Promise<CryptoKey> {
  const passwordKey = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(password),
    "PBKDF2",
    false,
    ["deriveKey"]
  );
  return crypto.subtle.deriveKey(
    {
      name: "PBKDF2",
      hash: "SHA-256",
      salt: toArrayBuffer(salt),
      iterations: KDF_ITERATIONS
    },
    passwordKey,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"]
  );
}

function randomBytes(length: number): Uint8Array {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
  return bytes;
}

function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

function bytesToBase64(bytes: Uint8Array): string {
  let raw = "";
  for (const byte of bytes) {
    raw += String.fromCharCode(byte);
  }
  return btoa(raw);
}

function base64ToBytes(value: string): Uint8Array {
  return Uint8Array.from(atob(value), (char) => char.charCodeAt(0));
}

async function readVaultRecord(): Promise<VaultRecord | null> {
  return (await withStore("readonly", (store) => store.get(RECORD_ID))) ?? null;
}

async function writeVaultRecord(record: VaultRecord): Promise<void> {
  await withStore("readwrite", (store) => store.put(record));
}

async function withStore<T>(
  mode: IDBTransactionMode,
  action: (store: IDBObjectStore) => IDBRequest<T>
): Promise<T> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, mode);
    const request = action(tx.objectStore(STORE_NAME));
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed."));
    tx.oncomplete = () => db.close();
    tx.onerror = () => {
      db.close();
      reject(tx.error ?? new Error("IndexedDB transaction failed."));
    };
  });
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => {
      request.result.createObjectStore(STORE_NAME, { keyPath: "id" });
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB open failed."));
  });
}
