import { Badge } from "@/components/ui/badge";
import { statusTone } from "@/lib/format";

export function StatusBadge({ status }: { status: string }) {
  return <Badge variant={statusTone(status)}>{status}</Badge>;
}
