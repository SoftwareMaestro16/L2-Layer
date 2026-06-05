import { SearchBar } from "@/components/search-bar";

export function ExplorerShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-dvh">
      <SearchBar />
      <main className="mx-auto w-full max-w-7xl px-4 py-6">{children}</main>
    </div>
  );
}
