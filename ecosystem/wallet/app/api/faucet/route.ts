import { NextResponse, type NextRequest } from "next/server";

const DEFAULT_API_BASE = "http://127.0.0.1:8080";

export async function POST(request: NextRequest) {
  const adminToken = process.env.ENTROPIS_ADMIN_TOKEN ?? process.env.L2_ADMIN_TOKEN;
  if (!adminToken) {
    return NextResponse.json({ error: "ENTROPIS_ADMIN_TOKEN or L2_ADMIN_TOKEN is required" }, { status: 503 });
  }

  const body = (await request.json()) as { account_id?: unknown };
  if (typeof body.account_id !== "string") {
    return NextResponse.json({ error: "account_id is required" }, { status: 400 });
  }

  const apiBase = (process.env.ENTROPIS_API_URL ?? DEFAULT_API_BASE).replace(/\/+$/u, "");
  const faucet = await fetch(`${apiBase}/v1/admin/faucet/ent`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${adminToken}`,
      "content-type": "application/json"
    },
    body: JSON.stringify({ account_id: body.account_id })
  });

  const faucetText = await faucet.text();
  if (!faucet.ok) {
    return new NextResponse(faucetText, { status: faucet.status });
  }

  await fetch(`${apiBase}/v1/admin/produce-block`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${adminToken}`,
      "content-type": "application/json"
    },
    body: "{}"
  }).catch(() => undefined);

  return new NextResponse(faucetText, {
    status: faucet.status,
    headers: { "content-type": "application/json" }
  });
}
