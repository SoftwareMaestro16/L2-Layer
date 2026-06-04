"use client";

import Image from "next/image";
import { FormEvent, useState } from "react";
import { KeyRound, Plus, ShieldCheck, Wallet } from "lucide-react";
import { ZodError } from "zod";
import { seedImportSchema } from "@/lib/schemas";
import { useWalletStore } from "@/store/wallet-store";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";

export function AuthPanel() {
  const createWallet = useWalletStore((state) => state.createWallet);
  const importWallet = useWalletStore((state) => state.importWallet);
  const [seedPhrase, setSeedPhrase] = useState("");
  const [seedError, setSeedError] = useState<string | null>(null);

  function handleImport(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSeedError(null);

    try {
      seedImportSchema.parse({ seedPhrase });
      setSeedPhrase("");
      importWallet();
    } catch (error) {
      if (error instanceof ZodError) {
        setSeedError(error.issues[0]?.message ?? "Invalid mock seed phrase.");
      } else {
        setSeedError("Invalid mock seed phrase.");
      }
    }
  }

  return (
    <div className="mx-auto grid min-h-screen w-full max-w-6xl items-center gap-8 px-4 py-8 lg:grid-cols-[0.9fr_1.1fr]">
      <section className="space-y-7">
        <div className="flex items-center gap-3">
          <div className="flex h-12 w-12 items-center justify-center rounded-lg border bg-card">
            <Image src="/entropis.png" alt="Entropis" width={34} height={34} priority />
          </div>
          <div>
            <h1 className="text-2xl font-semibold">EnWallet</h1>
            <p className="text-sm text-muted-foreground">Entropis mock wallet</p>
          </div>
        </div>

        <div className="space-y-4">
          <Badge variant="warning">UI prototype</Badge>
          <h2 className="max-w-xl text-4xl font-semibold leading-tight text-foreground">
            Create or import a demo EnWallet without touching real keys.
          </h2>
          <p className="max-w-xl text-base leading-7 text-muted-foreground">
            This screen models the EnWallet experience while blockchain logic, seed generation, and signing stay
            out of scope.
          </p>
        </div>

        <div className="grid gap-3 sm:grid-cols-3">
          {[
            ["No seed storage", ShieldCheck],
            ["Mock balances", Wallet],
            ["Local UI state", KeyRound]
          ].map(([label, Icon]) => (
            <div key={label as string} className="rounded-lg border bg-card p-4">
              <Icon className="mb-3 h-5 w-5 text-primary" />
              <p className="text-sm font-semibold">{label as string}</p>
            </div>
          ))}
        </div>
      </section>

      <Card className="w-full">
        <CardHeader>
          <CardTitle>Access wallet</CardTitle>
          <CardDescription>Both paths create the same mocked EnWallet session.</CardDescription>
        </CardHeader>
        <CardContent>
          <Tabs defaultValue="create" className="w-full">
            <TabsList className="grid w-full grid-cols-2">
              <TabsTrigger value="create">Create wallet</TabsTrigger>
              <TabsTrigger value="import">Import seed</TabsTrigger>
            </TabsList>

            <TabsContent value="create" className="space-y-4">
              <div className="rounded-lg border bg-muted/50 p-4">
                <p className="text-sm font-semibold">Demo account</p>
                <p className="mt-1 text-sm text-muted-foreground">
                  Creates a fixed mock address, balance, and transaction history for UI testing.
                </p>
              </div>
              <Button className="w-full" onClick={createWallet}>
                <Plus className="h-4 w-4" />
                Create mock wallet
              </Button>
            </TabsContent>

            <TabsContent value="import">
              <form className="space-y-4" onSubmit={handleImport}>
                <div className="space-y-2">
                  <Label htmlFor="seedPhrase">Seed phrase</Label>
                  <Textarea
                    id="seedPhrase"
                    value={seedPhrase}
                    onChange={(event) => setSeedPhrase(event.target.value)}
                    placeholder="twelve mock words are accepted here for visual testing only"
                    spellCheck={false}
                    autoComplete="off"
                  />
                  <p className="text-xs text-muted-foreground">
                    The phrase is validated for length, then cleared. It is never stored or used for derivation.
                  </p>
                  {seedError ? <p className="text-sm font-semibold text-destructive">{seedError}</p> : null}
                </div>
                <Button className="w-full" type="submit">
                  <KeyRound className="h-4 w-4" />
                  Open mock wallet
                </Button>
              </form>
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>
    </div>
  );
}
