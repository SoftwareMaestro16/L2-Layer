"use client";

import { FormEvent, useMemo, useState } from "react";
import { useQueries, useQueryClient } from "@tanstack/react-query";
import { LockKeyhole, RefreshCw, ShieldAlert } from "lucide-react";
import { StatusBadge } from "@/components/status-badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { fetchAppJson } from "@/lib/api";

type OperatorPayload = Record<string, unknown>;

const resources = [
  "readiness",
  "metrics",
  "failures",
  "relayer",
  "finalizer",
  "signer",
  "faucet",
] as const;

export function OperatorPanel() {
  const queryClient = useQueryClient();
  const [password, setPassword] = useState("");
  const [authError, setAuthError] = useState<string | null>(null);
  const operatorQueries = useQueries({
    queries: resources.map((resource) => ({
      queryKey: ["operator", resource],
      queryFn: () => fetchAppJson<OperatorPayload>(`/api/operator/${resource}`),
      enabled: false,
      retry: false,
    })),
  });
  const authenticated = operatorQueries.some((query) => query.data);

  async function login(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setAuthError(null);
    try {
      await fetchAppJson("/api/operator/login", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ password }),
      });
      await Promise.all(operatorQueries.map((query) => query.refetch()));
      setPassword("");
    } catch (error) {
      setAuthError(error instanceof Error ? error.message : "operator login failed");
    }
  }

  async function logout() {
    await fetchAppJson("/api/operator/logout", { method: "POST" }).catch(() => undefined);
    await queryClient.resetQueries({ queryKey: ["operator"] });
  }

  const summaries = useMemo(
    () =>
      operatorQueries.map((query, index) => ({
        resource: resources[index],
        data: query.data,
        error: query.error,
        fetching: query.isFetching,
      })),
    [operatorQueries],
  );

  return (
    <Card className="border-white/10 bg-white/[0.05]">
      <CardHeader className="flex flex-row items-center justify-between gap-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <ShieldAlert className="h-4 w-4 text-amber-300" />
          Operator dashboard
        </CardTitle>
        {authenticated ? (
          <div className="flex gap-2">
            <Button size="sm" variant="secondary" onClick={() => operatorQueries.forEach((query) => query.refetch())}>
              <RefreshCw className="h-4 w-4" />
              Refresh
            </Button>
            <Button size="sm" variant="outline" onClick={logout}>
              Logout
            </Button>
          </div>
        ) : null}
      </CardHeader>
      <CardContent className="space-y-4">
        {!authenticated ? (
          <form className="grid gap-2 sm:grid-cols-[1fr_auto]" onSubmit={login}>
            <div className="relative">
              <LockKeyhole className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-500" />
              <Input
                className="pl-9"
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                placeholder="Operator password"
                autoComplete="current-password"
              />
            </div>
            <Button type="submit" disabled={password.length === 0}>
              Unlock operator view
            </Button>
            {authError ? <p className="text-sm font-semibold text-red-300 sm:col-span-2">{authError}</p> : null}
          </form>
        ) : null}

        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          {summaries.map((item) => (
            <OperatorCard
              data={item.data}
              error={item.error}
              fetching={item.fetching}
              key={item.resource}
              resource={item.resource}
            />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function OperatorCard({
  resource,
  data,
  error,
  fetching,
}: {
  resource: string;
  data?: OperatorPayload;
  error: unknown;
  fetching: boolean;
}) {
  const status = statusFromPayload(data, error, fetching);
  return (
    <div className="min-h-32 rounded-md border border-white/10 bg-black/20 p-3">
      <div className="flex items-center justify-between gap-2">
        <span className="text-sm font-semibold capitalize">{resource.replace("-", " ")}</span>
        <StatusBadge status={status} />
      </div>
      <pre className="mt-3 max-h-40 overflow-auto whitespace-pre-wrap break-words text-xs text-zinc-400">
        {data ? safeJson(data) : error ? safeError(error) : "locked"}
      </pre>
    </div>
  );
}

function statusFromPayload(data: unknown, error: unknown, fetching: boolean): string {
  if (fetching) {
    return "loading";
  }
  if (error) {
    return "locked";
  }
  if (data && typeof data === "object" && "status" in data && typeof data.status === "string") {
    return data.status;
  }
  return data ? "available" : "locked";
}

function safeJson(value: unknown): string {
  return JSON.stringify(value, null, 2).slice(0, 1200);
}

function safeError(error: unknown): string {
  return error && typeof error === "object" && "message" in error ? String(error.message) : "request failed";
}
