"use client";

import { useMutation } from "@tanstack/react-query";
import { Upload } from "lucide-react";
import { useState } from "react";
import { ExplorerShell } from "@/components/explorer-shell";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { submitVerifier } from "@/lib/api";
import { useApiStore } from "@/lib/store";

export default function VerifierPage() {
  const apiBase = useApiStore((state) => state.apiBase);
  const [accountId, setAccountId] = useState("");
  const [codeHash, setCodeHash] = useState("");
  const [files, setFiles] = useState<Array<{ path: string; content: string }>>([]);
  const mutation = useMutation({
    mutationFn: () =>
      submitVerifier(apiBase, {
        account_id: accountId || undefined,
        code_hash: codeHash || undefined,
        files
      })
  });

  async function pickFiles(event: React.ChangeEvent<HTMLInputElement>) {
    const selected = Array.from(event.currentTarget.files ?? []).filter((file) => file.name.endsWith(".tolk"));
    const next = await Promise.all(selected.map(async (file) => ({ path: file.name, content: await file.text() })));
    setFiles(next);
  }

  return (
    <ExplorerShell>
      <Card className="mx-auto max-w-3xl">
        <CardHeader>
          <CardTitle>Contract Source Verifier</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-zinc-400">
            Upload Tolk source files for a contract account or code hash. Submissions stay pending until an operator verifies the source against bytecode.
          </p>
          <Input value={accountId} onChange={(e) => setAccountId(e.currentTarget.value)} placeholder="Contract account address" />
          <Input value={codeHash} onChange={(e) => setCodeHash(e.currentTarget.value)} placeholder="Or 64-char code hash" />
          <Input type="file" accept=".tolk" multiple onChange={pickFiles} />
          <div className="rounded-lg border border-white/10 bg-white/[0.04] p-3 text-sm text-zinc-300">
            {files.length ? files.map((file) => <p key={file.path}>{file.path}</p>) : "No .tolk files selected."}
          </div>
          <Button disabled={!files.length || mutation.isPending} onClick={() => mutation.mutate()}>
            <Upload className="h-4 w-4" />
            Submit for verification
          </Button>
          {mutation.data ? <p className="text-sm text-violet-200">Submission status: {mutation.data.status}</p> : null}
          {mutation.error ? <p className="text-sm text-red-300">{mutation.error.message}</p> : null}
        </CardContent>
      </Card>
    </ExplorerShell>
  );
}
