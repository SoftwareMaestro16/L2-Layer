import { redirect } from "next/navigation";

export default async function TransactionRedirectPage({
  searchParams,
}: {
  searchParams: Promise<{ q?: string }>;
}) {
  const { q } = await searchParams;
  const hash = q?.trim().replace(/^0x/, "");
  if (!hash) {
    redirect("/");
  }
  redirect(`/transaction/${encodeURIComponent(hash)}`);
}
