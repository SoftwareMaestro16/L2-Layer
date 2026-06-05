import * as React from "react";
import { cn } from "@/lib/utils";

export function Badge({ className, ...props }: React.HTMLAttributes<HTMLSpanElement>) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md border border-violet-300/20 bg-violet-400/15 px-2 py-0.5 text-xs font-semibold text-violet-100",
        className
      )}
      {...props}
    />
  );
}
