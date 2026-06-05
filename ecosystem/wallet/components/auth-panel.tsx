"use client";

import Image from "next/image";
import { FormEvent, useEffect, useState } from "react";
import { Copy, KeyRound, Lock, Plus, ShieldCheck, Wallet } from "lucide-react";
import { ZodError } from "zod";
import { createMnemonic24 } from "@/lib/enwallet";
import { seedImportSchema } from "@/lib/schemas";
import { useWalletStore } from "@/store/wallet-store";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";

export function AuthPanel() {
  const importWallet = useWalletStore((state) => state.importWallet);
  const openStoredWallet = useWalletStore((state) => state.openStoredWallet);
  const refreshStoredWallet = useWalletStore((state) => state.refreshStoredWallet);
  const hasStoredWallet = useWalletStore((state) => state.hasStoredWallet);
  const [generatedSeed, setGeneratedSeed] = useState("");
  const [seedPhrase, setSeedPhrase] = useState("");
  const [openPassword, setOpenPassword] = useState("");
  const [createPassword, setCreatePassword] = useState("");
  const [importPassword, setImportPassword] = useState("");
  const [backupConfirmed, setBackupConfirmed] = useState(false);
  const [seedError, setSeedError] = useState<string | null>(null);
  const [unlockError, setUnlockError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void refreshStoredWallet();
  }, [refreshStoredWallet]);

  function handleGenerate() {
    setGeneratedSeed(createMnemonic24());
    setSeedError(null);
  }

  async function handleOpenStored() {
    setBusy(true);
    setUnlockError(null);
    const result = await openStoredWallet(openPassword);
    setBusy(false);
    if (!result.ok) {
      setUnlockError(result.message);
    }
  }

  async function handleOpenGenerated() {
    setSeedError(null);
    if (!backupConfirmed) {
      setSeedError("Confirm that you saved the recovery phrase before opening the wallet.");
      return;
    }
    setBusy(true);
    const result = await importWallet(generatedSeed, createPassword, "created");
    setBusy(false);
    if (!result.ok) {
      setSeedError(result.message);
      return;
    }
    setGeneratedSeed("");
    setCreatePassword("");
  }

  async function handleImport(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSeedError(null);
    setBusy(true);

    try {
      const parsed = seedImportSchema.parse({ seedPhrase });
      const result = await importWallet(parsed.seedPhrase, importPassword, "imported");
      if (!result.ok) {
        setSeedError(result.message);
      } else {
        setSeedPhrase("");
        setImportPassword("");
      }
    } catch (error) {
      if (error instanceof ZodError) {
        setSeedError(error.issues[0]?.message ?? "Invalid seed phrase.");
      } else {
        setSeedError("Invalid seed phrase.");
      }
    } finally {
      setBusy(false);
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
            and keeps the recovery phrase in an encrypted IndexedDB vault when locked.
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
          {hasStoredWallet ? (
            <div className="space-y-2 rounded-lg border bg-muted/40 p-3">
              <Label htmlFor="openPassword">Unlock saved wallet</Label>
              <div className="grid gap-2 sm:grid-cols-[1fr_auto]">
                <Input
                  id="openPassword"
                  type="password"
                  value={openPassword}
                  onChange={(event) => setOpenPassword(event.target.value)}
                  placeholder="Vault password"
                  autoComplete="current-password"
                />
                <Button variant="secondary" onClick={handleOpenStored} disabled={busy || openPassword.length < 8}>
                  <Lock className="h-4 w-4" />
                  Unlock
                </Button>
              </div>
              {unlockError ? <p className="text-sm font-semibold text-destructive">{unlockError}</p> : null}
            </div>
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
                  Save these 24 words before opening the wallet. They cannot be recovered from the encrypted vault
                  without your password.
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
              <label className="flex items-start gap-2 rounded-lg border bg-card p-3 text-sm">
                <input
                  className="mt-1"
                  type="checkbox"
                  checked={backupConfirmed}
                  onChange={(event) => setBackupConfirmed(event.target.checked)}
                />
                <span>I saved the recovery phrase offline and understand it is not recoverable.</span>
              </label>
              <div className="space-y-2">
                <Label htmlFor="createPassword">Vault password</Label>
                <Input
                  id="createPassword"
                  type="password"
                  value={createPassword}
                  onChange={(event) => setCreatePassword(event.target.value)}
                  placeholder="At least 8 characters"
                  autoComplete="new-password"
                />
              </div>
              <Button
                className="w-full"
                onClick={handleOpenGenerated}
                disabled={!generatedSeed || createPassword.length < 8 || busy}
              >
                <Wallet className="h-4 w-4" />
                Encrypt and open wallet
              </Button>
              {seedError ? <p className="text-sm font-semibold text-destructive">{seedError}</p> : null}
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
                    The phrase is validated with the English BIP39 wordlist and encrypted before local storage.
                  </p>
                  {seedError ? <p className="text-sm font-semibold text-destructive">{seedError}</p> : null}
                </div>
                <div className="space-y-2">
                  <Label htmlFor="importPassword">Vault password</Label>
                  <Input
                    id="importPassword"
                    type="password"
                    value={importPassword}
                    onChange={(event) => setImportPassword(event.target.value)}
                    placeholder="At least 8 characters"
                    autoComplete="new-password"
                  />
                </div>
                <Button className="w-full" type="submit" disabled={busy || importPassword.length < 8}>
                  <KeyRound className="h-4 w-4" />
                  Encrypt and open EnWallet
                </Button>
              </form>
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>
    </div>
  );
}
