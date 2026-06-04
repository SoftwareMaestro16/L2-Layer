import { shortHash } from "@/lib/format";

export function HashText({
  value,
  className = "",
  full = false,
}: {
  value?: string | null;
  className?: string;
  full?: boolean;
}) {
  return (
    <span
      className={`font-mono text-xs tabular-nums text-violet-200 ${className}`}
      title={value ?? undefined}
    >
      {full ? value || "-" : shortHash(value)}
    </span>
  );
}
