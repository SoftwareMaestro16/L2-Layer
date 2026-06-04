import { NextResponse } from "next/server";

const DEFAULT_API_BASE = "http://127.0.0.1:8080";

export async function POST() {
  const adminToken = process.env.ENTROPIS_ADMIN_TOKEN ?? process.env.L2_ADMIN_TOKEN;
  if (!adminToken) {
    return NextResponse.json({ error: "admin token is not configured" }, { status: 503 });
  }

  const apiBase = (process.env.ENTROPIS_API_URL ?? DEFAULT_API_BASE).replace(/\/+$/u, "");
  const response = await fetch(`${apiBase}/v1/admin/produce-block`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${adminToken}`,
      "content-type": "application/json"
    },
    body: "{}"
  });

  const text = await response.text();
  return new NextResponse(text, {
    status: response.status,
    headers: text ? { "content-type": "application/json" } : undefined
  });
}
