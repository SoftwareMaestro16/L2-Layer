import { redirect } from "next/navigation";

export default async function AccountLookupPage({
  searchParams
}: {
  searchParams: Promise<{ q?: string }>;
}) {
  const { q } = await searchParams;
  const value = q?.trim();
  redirect(value ? `/account/${encodeURIComponent(value)}` : "/");
}
