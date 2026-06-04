"use client";

import Image from "next/image";
import { FormEvent, useState } from "react";
import { Copy, KeyRound, Plus, ShieldCheck, Wallet } from "lucide-react";
import { ZodError } from "zod";
import { createMnemonic24 } from "@/lib/enwallet";
import { seedImportSchema } from "@/lib/schemas";
import { checkStoredWallet, useWalletStore } from "@/store/wallet-store";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";

export function AuthPanel() {
  const importWallet = useWalletStore((state) => state.importWallet);
  const openStoredWallet = useWalletStore((state) => state.openStoredWallet);
  const [generatedSeed, setGeneratedSeed] = useState("");
  const [seedPhrase, setSeedPhrase] = useState("");
  const [seedError, setSeedError] = useState<string | null>(null);
  const [hasStored] = useState(() => typeof window !== "undefined" && checkStoredWallet());
  const [copied, setCopied] = useState(false);

  function handleGenerate() {
    setGeneratedSeed(createMnemonic24());
    setSeedError(null);
  }

  function handleOpenGenerated() {
    importWallet(generatedSeed, "created");
  }

  function handleImport(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSeedError(null);

    try {
      const parsed = seedImportSchema.parse({ seedPhrase });
      setSeedPhrase("");
      importWallet(parsed.seedPhrase, "imported");
    } catch (error) {
      if (error instanceof ZodError) {
        setSeedError(error.issues[0]?.message ?? "Invalid seed phrase.");
      } else {
        setSeedError("Invalid seed phrase.");
      }
    }
  }

  async function copySeed() {
    await navigator.clipboard.writeText(generatedSeed);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
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
            <p className="text-sm text-muted-foreground">Entropis L2 testnet wallet</p>
          </div>
        </div>

        <div className="space-y-4">
          <Badge variant="warning">Testnet only</Badge>
          <h2 className="max-w-xl text-4xl font-semibold leading-tight text-foreground">
            Create a real Entropis L2 wallet from a 24-word seed phrase.
          </h2>
          <p className="max-w-xl text-base leading-7 text-muted-foreground">
            EnWallet derives an Ed25519 keypair locally, builds an EX address, signs L2 transactions in the browser,
            and stores the seed phrase only in this browser&apos;s local storage for now.
          </p>
        </div>

        <div className="grid gap-3 sm:grid-cols-3">
          {[
            ["24-word BIP39", ShieldCheck],
            ["EX address", Wallet],
            ["Local signing", KeyRound]
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
          <CardDescription>Create a new seed, import an existing seed, or reopen the saved local wallet.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {hasStored ? (
            <Button className="w-full" variant="secondary" onClick={openStoredWallet}>
              <Wallet className="h-4 w-4" />
              Open saved wallet
            </Button>
          ) : null}

          <Tabs defaultValue="create" className="w-full">
            <TabsList className="grid w-full grid-cols-2">
              <TabsTrigger value="create">Create wallet</TabsTrigger>
              <TabsTrigger value="import">Import seed</TabsTrigger>
            </TabsList>

            <TabsContent value="create" className="space-y-4">
              <div className="rounded-lg border bg-muted/50 p-4">
                <p className="text-sm font-semibold">Recovery phrase</p>
                <p className="mt-1 text-sm text-muted-foreground">
                  Save these 24 words before opening the wallet. This prototype stores them in localStorage.
                </p>
              </div>

              {generatedSeed ? (
                <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                  {generatedSeed.split(" ").map((word, index) => (
                    <div key={`${word}-${index}`} className="rounded-md border bg-card px-3 py-2 text-sm">
                      <span className="mr-2 text-xs text-muted-foreground">{index + 1}</span>
                      <span className="font-semibold">{word}</span>
                    </div>
                  ))}
                </div>
              ) : null}

              <div className="grid gap-2 sm:grid-cols-2">
                <Button variant="outline" onClick={handleGenerate}>
                  <Plus className="h-4 w-4" />
                  Generate 24 words
                </Button>
                <Button variant="outline" onClick={copySeed} disabled={!generatedSeed}>
                  <Copy className="h-4 w-4" />
                  {copied ? "Copied" : "Copy seed"}
                </Button>
              </div>
              <Button className="w-full" onClick={handleOpenGenerated} disabled={!generatedSeed}>
                <Wallet className="h-4 w-4" />
                I saved it, open wallet
              </Button>
            </TabsContent>

            <TabsContent value="import">
              <form className="space-y-4" onSubmit={handleImport}>
                <div className="space-y-2">
                  <Label htmlFor="seedPhrase">24-word seed phrase</Label>
                  <Textarea
                    id="seedPhrase"
                    value={seedPhrase}
                    onChange={(event) => setSeedPhrase(event.target.value)}
                    placeholder="enter 24 BIP39 words"
                    spellCheck={false}
                    autoComplete="off"
                  />
                  <p className="text-xs text-muted-foreground">
                    The phrase is validated with the English BIP39 wordlist and stored locally for testnet use.
                  </p>
                  {seedError ? <p className="text-sm font-semibold text-destructive">{seedError}</p> : null}
                </div>
                <Button className="w-full" type="submit">
                  <KeyRound className="h-4 w-4" />
                  Open EnWallet
                </Button>
              </form>
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>
    </div>
  );
}
