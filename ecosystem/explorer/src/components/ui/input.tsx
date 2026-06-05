import * as React from "react";
import { cn } from "@/lib/utils";

export const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  function Input({ className, ...props }, ref) {
  return (
    <input
      ref={ref}
      className={cn(
        "h-11 w-full min-w-0 rounded-lg border border-white/10 bg-white/[0.07] px-3 text-sm text-white outline-none placeholder:text-zinc-500 focus:border-violet-300 focus:ring-2 focus:ring-violet-400/30",
        className
      )}
      {...props}
    />
  );
  }
);
