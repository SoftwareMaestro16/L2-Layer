import { ReactNode } from "react";
import { SearchBar } from "@/components/search-bar";

export function ExplorerShell({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-dvh text-zinc-100">
      <SearchBar />
      <main className="mx-auto w-full max-w-7xl px-4 py-6 md:px-6">
        {children}
      </main>
    </div>
  );
}
