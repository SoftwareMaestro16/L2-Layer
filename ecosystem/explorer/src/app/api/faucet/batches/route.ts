import { NextResponse } from "next/server";

export async function GET() {
  const faucetApi = process.env.FAUCET_API_URL?.replace(/\/+$/u, "");
  if (!faucetApi) {
    return NextResponse.json({ error: "faucet backend is not configured" }, { status: 503 });
  }
  const response = await fetch(`${faucetApi}/api/faucet/batches`, {
    headers: { accept: "application/json" },
    cache: "no-store",
  });
  const text = await response.text();
  const body = text ? safeJson(text) : null;
  if (!response.ok) {
    return NextResponse.json({ error: safeError(body) }, { status: response.status });
  }
  return NextResponse.json(body);
}

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function safeError(body: unknown): string {
  if (body && typeof body === "object" && "error" in body && typeof body.error === "string") {
    return body.error;
  }
  return "faucet request failed";
}
