"use client";

import { FormEvent, useState } from "react";
import { useRouter } from "next/navigation";
import { Search, Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useExplorerSettings } from "@/lib/settings";
import { isProbablyHash } from "@/lib/format";

export function SearchBar() {
  const router = useRouter();
  const { apiBase, setApiBase } = useExplorerSettings();
  const [value, setValue] = useState("");
  const [apiInput, setApiInput] = useState(apiBase);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    openAccount();
  }

  function openAccount() {
    const next = value.trim();
    if (next) router.push(`/account/${encodeURIComponent(next)}`);
  }

  function openTransaction() {
    const next = value.trim().replace(/^0x/, "");
    if (next) router.push(`/transaction/${encodeURIComponent(next)}`);
  }

  return (
    <div className="sticky top-0 z-20 border-b border-white/10 bg-zinc-950/92 backdrop-blur">
      <div className="mx-auto flex min-h-16 w-full max-w-7xl flex-col gap-3 px-4 py-3 md:flex-row md:items-center md:px-6">
        <button
          className="flex items-center gap-3 text-left"
          type="button"
          onClick={() => router.push("/")}
        >
          <span className="grid h-9 w-9 place-items-center rounded-md bg-emerald-400 text-sm font-bold text-zinc-950">
            E
          </span>
          <span>
            <span className="block text-base font-semibold text-zinc-50">
              Entropis Explorer
            </span>
            <span className="block text-xs text-zinc-400">testnet</span>
          </span>
        </button>

        <form className="flex min-w-0 flex-1 gap-2" onSubmit={submit}>
          <div className="relative min-w-0 flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-500" />
            <Input
              className="h-10 border-white/10 bg-black/30 pl-9 font-mono text-sm"
              spellCheck={false}
              value={value}
              onChange={(event) => setValue(event.target.value)}
              placeholder="address or transaction hash"
            />
          </div>
          <Button className="h-10" type="submit">
            Account
          </Button>
          <Button
            className="h-10"
            disabled={value.trim() !== "" && !isProbablyHash(value)}
            type="button"
            variant="secondary"
            onClick={openTransaction}
          >
            Tx
          </Button>
        </form>

        <form
          className="flex min-w-0 gap-2 md:w-[22rem]"
          onSubmit={(event) => {
            event.preventDefault();
            setApiBase(apiInput);
          }}
        >
          <div className="relative min-w-0 flex-1">
            <Settings className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-500" />
            <Input
              className="h-10 border-white/10 bg-black/30 pl-9 text-xs"
              value={apiInput}
              onChange={(event) => setApiInput(event.target.value)}
              onBlur={() => setApiBase(apiInput)}
            />
          </div>
        </form>
      </div>
    </div>
  );
}
