import { shortHash } from "@/lib/format";
import { cn } from "@/lib/utils";

export function HashText({ value, className }: { value: string; className?: string }) {
  return (
    <span title={value} className={cn("font-mono text-violet-200", className)}>
      {shortHash(value, 10, 8)}
    </span>
  );
}
