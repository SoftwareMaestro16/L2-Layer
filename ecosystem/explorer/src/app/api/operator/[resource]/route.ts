import { NextResponse } from "next/server";
import {
  fetchOperatorResource,
  operatorResources,
  type OperatorResource,
} from "@/lib/operator-proxy";

export async function GET(
  _request: Request,
  { params }: { params: Promise<{ resource: string }> },
) {
  const { resource } = await params;
  if (!isOperatorResource(resource)) {
    return NextResponse.json({ error: "unsupported operator resource" }, { status: 404 });
  }
  const result = await fetchOperatorResource(resource);
  if (!result.ok) {
    return NextResponse.json({ error: result.error }, { status: result.status });
  }
  return NextResponse.json(result.body);
}

function isOperatorResource(value: string): value is OperatorResource {
  return value in operatorResources;
}
