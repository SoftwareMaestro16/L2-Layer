import { redirect } from "next/navigation";

export default async function TransactionLookupPage({
  searchParams
}: {
  searchParams: Promise<{ q?: string }>;
}) {
  const { q } = await searchParams;
  const value = q?.trim().replace(/^0x/u, "");
  redirect(value ? `/transaction/${encodeURIComponent(value)}` : "/");
}
