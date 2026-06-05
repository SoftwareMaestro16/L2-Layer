import { NextResponse } from "next/server";
import { clearOperatorSession } from "@/lib/operator-proxy";

export async function POST() {
  await clearOperatorSession();
  return NextResponse.json({ ok: true });
}
