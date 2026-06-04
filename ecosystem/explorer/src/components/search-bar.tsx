"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Eye, Settings } from "lucide-react";
import { Input } from "@/components/ui/input";
import { LookupForm } from "@/components/lookup-form";
import { useExplorerSettings } from "@/lib/settings";

export function SearchBar() {
  const router = useRouter();
  const { apiBase, setApiBase } = useExplorerSettings();
  const [apiInput, setApiInput] = useState(apiBase);

  return (
    <div className="sticky top-0 z-20 border-b border-white/10 bg-[#171720]/88 backdrop-blur-xl">
      <div className="mx-auto flex min-h-16 w-full max-w-7xl flex-col gap-3 px-4 py-3 md:flex-row md:items-center md:px-6">
        <button
          className="flex items-center gap-3 text-left"
          type="button"
          onClick={() => router.push("/")}
        >
          <span className="grid h-10 w-10 place-items-center rounded-lg bg-[linear-gradient(135deg,#2563eb,#7c3aed)] text-white shadow-lg shadow-violet-500/25">
            <Eye className="h-5 w-5" />
          </span>
          <span>
            <span className="block text-base font-semibold text-zinc-50">
              EnWatcher
            </span>
            <span className="block text-xs text-violet-200/70">Entropis testnet</span>
          </span>
        </button>

        <LookupForm />

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
              className="h-10 border-white/10 bg-white/[0.06] pl-9 text-xs"
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
