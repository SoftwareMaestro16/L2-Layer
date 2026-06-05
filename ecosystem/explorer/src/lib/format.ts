export function shortHash(value: string | null | undefined, head = 10): string {
  if (!value) return "-";
  if (value.length <= head + 8) return value;
  return `${value.slice(0, head)}...${value.slice(-8)}`;
}

export function formatAmount(value: string | null | undefined): string {
  if (!value) return "-";
  return BigInt(value).toLocaleString("en-US");
}

export function formatUnixTime(value: number): string {
  return new Intl.DateTimeFormat("en-US", {
    dateStyle: "medium",
    timeStyle: "medium",
    timeZone: "UTC",
  }).format(new Date(value * 1000));
}

export function statusTone(status: string): "default" | "secondary" | "destructive" {
  if (/reject|fail|error/i.test(status)) return "destructive";
  if (/pending|unknown|waiting/i.test(status)) return "secondary";
  return "default";
}

export function isProbablyHash(value: string): boolean {
  return /^(0x)?[0-9a-fA-F]{64}$/.test(value.trim());
}
