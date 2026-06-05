export function shortHash(value: string, left = 8, right = 6): string {
  if (value.length <= left + right + 3) return value;
  return `${value.slice(0, left)}...${value.slice(-right)}`;
}

export function formatBaseUnits(value: string | null | undefined, decimals = 9): string {
  if (!value) return "0";
  const padded = value.padStart(decimals + 1, "0");
  const whole = padded.slice(0, -decimals);
  const fraction = padded.slice(-decimals).replace(/0+$/u, "");
  return fraction ? `${Number(whole).toLocaleString("en-US")}.${fraction}` : Number(whole).toLocaleString("en-US");
}

export function formatTime(seconds: number): string {
  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit"
  }).format(new Date(seconds * 1000));
}

export function isProbablyHash(value: string): boolean {
  return /^(0x)?[a-fA-F0-9]{64}$/u.test(value.trim());
}

export function enwalletSendLink(account: string, assetId = 0, amount?: string): string {
  const base = (process.env.NEXT_PUBLIC_ENWALLET_URL ?? "http://127.0.0.1:3001").replace(/\/+$/u, "");
  const params = new URLSearchParams({ to: account, asset_id: String(assetId) });
  if (amount) params.set("amount", amount);
  return `${base}/send?${params}`;
}
