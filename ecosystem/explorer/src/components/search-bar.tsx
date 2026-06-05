"use client";

import Link from "next/link";
import { Eye, Settings } from "lucide-react";
import { LookupForm } from "@/components/lookup-form";
import { Input } from "@/components/ui/input";
import { useApiStore } from "@/lib/store";

export function SearchBar() {
  const apiBase = useApiStore((state) => state.apiBase);
  const setApiBase = useApiStore((state) => state.setApiBase);

  return (
    <header className="sticky top-0 z-20 border-b border-white/10 bg-[#160b20]/85 px-4 py-3 backdrop-blur-xl">
      <div className="mx-auto flex max-w-7xl items-center gap-3">
        <Link href="/" className="flex min-w-fit items-center gap-3">
          <span className="flex h-10 w-10 items-center justify-center rounded-lg bg-[linear-gradient(135deg,#2563eb,#7c3aed)] shadow-lg shadow-violet-500/30">
            <Eye className="h-5 w-5" />
          </span>
          <span className="hidden sm:block">
            <span className="block text-base font-bold">EnWatcher</span>
            <span className="text-xs text-zinc-400">Entropis testnet</span>
          </span>
        </Link>
        <LookupForm />
        <div className="hidden w-64 items-center gap-2 rounded-lg border border-white/10 bg-white/[0.06] px-3 lg:flex">
          <Settings className="h-4 w-4 text-zinc-500" />
          <Input
            className="h-9 border-0 bg-transparent px-0 focus:border-0 focus:ring-0"
            value={apiBase}
            onChange={(event) => setApiBase(event.currentTarget.value)}
            aria-label="API base URL"
          />
        </div>
      </div>
    </header>
  );
}
