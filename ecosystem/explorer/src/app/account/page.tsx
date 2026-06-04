import { redirect } from "next/navigation";

export default async function AccountRedirectPage({
  searchParams,
}: {
  searchParams: Promise<{ q?: string }>;
}) {
  const { q } = await searchParams;
  const address = q?.trim();
  if (!address) {
    redirect("/");
  }
  redirect(`/account/${encodeURIComponent(address)}`);
}
