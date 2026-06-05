"use client";

import { KeyboardEvent, useRef, useState } from "react";
import { usePathname, useRouter } from "next/navigation";
import { ArrowRight, Search } from "lucide-react";
import { Input } from "@/components/ui/input";
import { isProbablyHash } from "@/lib/format";
import { cn } from "@/lib/utils";

type LookupFormProps = {
  variant?: "compact" | "hero";
};

export function LookupForm({ variant = "compact" }: LookupFormProps) {
  const pathname = usePathname();
  const routeValue = lookupValueFromPath(pathname);

  return (
    <LookupFormInner
      key={`${pathname}:${routeValue}`}
      initialValue={routeValue}
      variant={variant}
    />
  );
}

function LookupFormInner({
  initialValue,
  variant,
}: {
  initialValue: string;
  variant: "compact" | "hero";
}) {
  const router = useRouter();
  const inputRef = useRef<HTMLInputElement>(null);
  const [displayValue, setDisplayValue] = useState(initialValue);

  function openBestMatch(input: string) {
    const next = input.trim();
    if (!next) return;
    if (isProbablyHash(next)) {
      router.push(`/transaction/${encodeURIComponent(next.replace(/^0x/, ""))}`);
      return;
    }
    router.push(`/account/${encodeURIComponent(next)}`);
  }

  function openAccount() {
    const next = readInputValue().trim();
    if (next) router.push(`/account/${encodeURIComponent(next)}`);
  }

  function openTransaction() {
    const next = readInputValue().trim().replace(/^0x/, "");
    if (next) router.push(`/transaction/${encodeURIComponent(next)}`);
  }

  function readInputValue(): string {
    return inputRef.current?.value ?? displayValue;
  }

  function submitOnEnter(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter") return;
    event.preventDefault();
    openBestMatch(readInputValue());
  }

  const isHero = variant === "hero";
  const buttonMode = isProbablyHash(displayValue) ? "Tx" : "Account";

  return (
    <div
      role="search"
      className={cn(
        "flex w-full min-w-0 gap-2",
        isHero ? "mx-auto max-w-3xl flex-col sm:flex-row" : "flex-1",
      )}
    >
      <div className="relative min-w-0 flex-1">
        <Search
          className={cn(
            "pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-violet-200/70",
            isHero ? "h-5 w-5" : "h-4 w-4",
          )}
        />
        <Input
          ref={inputRef}
          className={cn(
            "border-white/10 bg-white/[0.06] font-mono shadow-inner shadow-black/20 placeholder:text-zinc-500 focus-visible:ring-violet-400/50",
            isHero
              ? "h-14 rounded-lg pl-12 text-base"
              : "h-10 rounded-lg pl-10 text-sm",
          )}
          spellCheck={false}
          defaultValue={initialValue}
          onInput={(event) => setDisplayValue(event.currentTarget.value)}
          onKeyDown={submitOnEnter}
          placeholder="Paste L2 address or transaction hash"
        />
      </div>
      <div className={cn("grid gap-2", isHero ? "grid-cols-2 sm:flex" : "grid-cols-2")}>
        <button
          className={cn(
            "inline-flex items-center justify-center gap-2 rounded-lg bg-cyan-600 text-sm font-semibold text-white transition hover:bg-cyan-500 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-300",
            isHero ? "h-14 px-6" : "h-10 px-4",
          )}
          type="button"
          onClick={() => openBestMatch(readInputValue())}
        >
          Open
          <ArrowRight className="h-4 w-4" />
        </button>
        <button
          className={cn(
            "inline-flex items-center justify-center rounded-lg border border-white/10 bg-white/[0.07] text-sm font-semibold text-zinc-100 transition hover:bg-white/[0.12] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-300",
            isHero ? "h-14 px-5" : "h-10 px-4",
          )}
          type="button"
          onClick={() => {
            if (buttonMode === "Tx") {
              openTransaction();
              return;
            }
            openAccount();
          }}
        >
          {buttonMode}
        </button>
      </div>
    </div>
  );
}

function lookupValueFromPath(pathname: string): string {
  const [kind, raw] = pathname.split("/").filter(Boolean);
  if ((kind === "account" || kind === "transaction") && raw) {
    return decodeURIComponent(raw);
  }
  return "";
}
