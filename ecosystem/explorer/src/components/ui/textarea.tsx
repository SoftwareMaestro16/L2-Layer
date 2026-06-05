import * as React from "react";
import { cn } from "@/lib/utils";

export function Textarea({ className, ...props }: React.TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <textarea
      className={cn(
        "min-h-32 w-full rounded-lg border border-white/10 bg-white/[0.07] px-3 py-2 text-sm text-white outline-none placeholder:text-zinc-500 focus:border-violet-300 focus:ring-2 focus:ring-violet-400/30",
        className
      )}
      {...props}
    />
  );
}
