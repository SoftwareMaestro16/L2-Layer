"use client";

import { FormEvent, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertCircle, Search } from "lucide-react";
import { JsonBlock } from "@/components/json-block";
import { StatusBadge } from "@/components/status-badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { fetchAppJson } from "@/lib/api";
import { useExplorerSettings } from "@/lib/settings";

type ResourceLookupProps = {
  title: string;
  label: string;
  placeholder: string;
  initialValue?: string;
  load: (apiBase: string, value: string) => Promise<unknown>;
};

export function ResourceLookup({
  title,
  label,
  placeholder,
  initialValue = "",
  load,
}: ResourceLookupProps) {
  const apiBase = useExplorerSettings((state) => state.apiBase);
  const [input, setInput] = useState(initialValue);
  const [submitted, setSubmitted] = useState(initialValue);
  const query = useQuery({
    queryKey: ["resource-lookup", title, apiBase, submitted],
    queryFn: () => load(apiBase, submitted),
    enabled: submitted.length > 0,
    retry: false,
  });

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitted(input.trim());
  }

  return (
    <div className="space-y-5">
      <Card className="border-white/10 bg-white/[0.05]">
        <CardHeader>
          <CardTitle className="text-base">{title}</CardTitle>
        </CardHeader>
        <CardContent>
          <form className="grid gap-2 sm:grid-cols-[1fr_auto]" onSubmit={submit}>
            <Input
              aria-label={label}
              value={input}
              onChange={(event) => setInput(event.target.value)}
              placeholder={placeholder}
              spellCheck={false}
            />
            <Button type="submit" disabled={!input.trim()}>
              <Search className="h-4 w-4" />
              Inspect
            </Button>
          </form>
        </CardContent>
      </Card>

      {query.error ? <LookupError error={query.error} /> : null}
      {query.data ? (
        <Card className="border-white/10 bg-white/[0.05]">
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle className="text-base">Result</CardTitle>
            <StatusBadge status={statusOf(query.data)} />
          </CardHeader>
          <CardContent>
            <JsonBlock value={query.data} />
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}

export async function getFaucetBatches(): Promise<unknown> {
  return fetchAppJson("/api/faucet/batches");
}

function LookupError({ error }: { error: unknown }) {
  const message = error && typeof error === "object" && "message" in error ? String(error.message) : "request failed";
  return (
    <Alert className="border-red-500/30 bg-red-950/30">
      <AlertCircle className="h-4 w-4" />
      <AlertTitle>Lookup failed</AlertTitle>
      <AlertDescription>{message}</AlertDescription>
    </Alert>
  );
}

function statusOf(value: unknown): string {
  if (value && typeof value === "object" && "status" in value && typeof value.status === "string") {
    return value.status;
  }
  return "available";
}
