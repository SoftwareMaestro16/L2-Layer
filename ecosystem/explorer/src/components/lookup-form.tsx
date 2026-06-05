"use client";

import { usePathname, useRouter } from "next/navigation";
import { ArrowRight, Search } from "lucide-react";
import { KeyboardEvent, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { isProbablyHash } from "@/lib/format";
import { cn } from "@/lib/utils";

export function LookupForm({ variant = "compact" }: { variant?: "compact" | "hero" }) {
  const pathname = usePathname();
  const initialValue = lookupValueFromPath(pathname);
  return <LookupFormInner key={`${pathname}:${initialValue}`} initialValue={initialValue} variant={variant} />;
}

function LookupFormInner({
  initialValue,
  variant
}: {
  initialValue: string;
  variant: "compact" | "hero";
}) {
  const router = useRouter();
  const inputRef = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState(initialValue);
  const isHero = variant === "hero";
  const mode = isProbablyHash(value) ? "Tx" : "Account";

  function readValue() {
    return (inputRef.current?.value ?? value).trim();
  }

  function openBestMatch() {
    const next = readValue();
    if (!next) return;
    if (isProbablyHash(next)) {
      router.push(`/transaction/${encodeURIComponent(next.replace(/^0x/u, ""))}`);
    } else {
      router.push(`/account/${encodeURIComponent(next)}`);
    }
  }

  function keyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter") {
      event.preventDefault();
      openBestMatch();
    }
  }

  return (
    <div role="search" className={cn("flex w-full min-w-0 gap-2", isHero ? "mx-auto max-w-3xl flex-col sm:flex-row" : "flex-1")}>
      <div className="relative min-w-0 flex-1">
        <Search className={cn("pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-violet-200/70", isHero ? "h-5 w-5" : "h-4 w-4")} />
        <Input
          ref={inputRef}
          className={cn("font-mono shadow-inner shadow-black/20", isHero ? "h-14 pl-12 text-base" : "h-10 pl-10 text-sm")}
          spellCheck={false}
          defaultValue={initialValue}
          onInput={(event) => setValue(event.currentTarget.value)}
          onKeyDown={keyDown}
          placeholder="Paste L2 address or transaction hash"
        />
      </div>
      <div className={cn("grid gap-2", isHero ? "grid-cols-2 sm:flex" : "grid-cols-2")}>
        <Button size={isHero ? "lg" : "sm"} type="button" onClick={openBestMatch}>
          Open <ArrowRight className="h-4 w-4" />
        </Button>
        <Button
          size={isHero ? "lg" : "sm"}
          variant="secondary"
          type="button"
          onClick={() => {
            const next = readValue();
            if (!next) return;
            router.push(mode === "Tx" ? `/transaction/${encodeURIComponent(next.replace(/^0x/u, ""))}` : `/account/${encodeURIComponent(next)}`);
          }}
        >
          {mode}
        </Button>
      </div>
    </div>
  );
}

function lookupValueFromPath(pathname: string): string {
  const [kind, raw] = pathname.split("/").filter(Boolean);
  return (kind === "account" || kind === "transaction") && raw ? decodeURIComponent(raw) : "";
}
