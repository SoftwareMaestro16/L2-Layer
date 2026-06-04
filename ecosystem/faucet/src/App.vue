<script setup lang="ts">
import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Coins,
  Copy,
  Loader2,
  RotateCcw,
  ShieldCheck,
  Sparkles,
} from "lucide-vue-next"
import { computed, ref } from "vue"

import Alert from "@/components/ui/Alert.vue"
import Badge from "@/components/ui/Badge.vue"
import Button from "@/components/ui/Button.vue"
import Card from "@/components/ui/Card.vue"
import Input from "@/components/ui/Input.vue"
import { addressSchema, claimMockEnt, FAUCET_AMOUNT, type FaucetClaim } from "@/lib/faucet"

type Status =
  | { kind: "idle" }
  | { kind: "success"; claim: FaucetClaim }
  | { kind: "cooldown"; claim: FaucetClaim; retryAt: number }
  | { kind: "network" }
  | { kind: "validation"; message: string }

const address = ref("")
const status = ref<Status>({ kind: "idle" })
const isLoading = ref(false)
const simulateFailure = ref(false)
const copied = ref(false)

const canCopy = computed(() => status.value.kind === "success" || status.value.kind === "cooldown")
const currentClaim = computed(() => {
  if (status.value.kind === "success" || status.value.kind === "cooldown") {
    return status.value.claim
  }

  return null
})

async function requestTokens() {
  copied.value = false
  const parsed = addressSchema.safeParse(address.value)

  if (!parsed.success) {
    status.value = { kind: "validation", message: parsed.error.issues[0]?.message ?? "Invalid address." }
    return
  }

  isLoading.value = true
  await delay(850)

  if (simulateFailure.value) {
    simulateFailure.value = false
    status.value = { kind: "network" }
    isLoading.value = false
    return
  }

  const result = claimMockEnt(parsed.data)
  status.value = result.ok
    ? { kind: "success", claim: result.claim }
    : { kind: "cooldown", claim: result.claim, retryAt: result.retryAt }
  isLoading.value = false
}

async function copyHash() {
  if (!currentClaim.value) return

  await navigator.clipboard.writeText(currentClaim.value.txHash)
  copied.value = true
}

function retry() {
  status.value = { kind: "idle" }
  void requestTokens()
}

function delay(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms))
}

function formatRetry(timestamp: number) {
  const seconds = Math.max(0, Math.ceil((timestamp - Date.now()) / 1000))
  return `${seconds}s`
}
</script>

<template>
  <main class="min-h-screen overflow-hidden bg-slate-950 text-slate-50">
    <div class="absolute inset-0 -z-10 faucet-bg" />
    <div class="mx-auto flex min-h-screen w-full max-w-6xl flex-col px-4 py-5 sm:px-6 lg:px-8">
      <header class="flex items-center justify-between gap-4">
        <div class="flex min-w-0 items-center gap-3">
          <img
            src="/entropis.png"
            alt="Entropis"
            class="size-10 rounded-lg border border-white/10 bg-slate-900 object-cover"
          />
          <div class="min-w-0">
            <p class="text-sm font-semibold text-slate-100">Entropis Faucet</p>
            <p class="truncate text-xs text-slate-400">Public testnet ENT grants</p>
          </div>
        </div>
        <Badge variant="violet">
          <ShieldCheck />
          Mock network
        </Badge>
      </header>

      <section class="grid flex-1 items-center gap-6 py-8 lg:grid-cols-[1.05fr_0.95fr] lg:py-10">
        <div class="space-y-5">
          <div class="max-w-2xl space-y-4">
            <div class="flex flex-wrap gap-2">
              <Badge variant="blue">3002 local</Badge>
              <Badge>100 ENT per claim</Badge>
            </div>
            <h1 class="text-4xl font-semibold leading-tight text-white sm:text-5xl">
              Claim test ENT for an Entropis L2 account.
            </h1>
            <p class="max-w-xl text-sm leading-6 text-slate-300 sm:text-base">
              Enter an account id, submit a request, and receive a mock grant with a test transaction.
            </p>
          </div>

          <div class="grid gap-3 sm:grid-cols-3">
            <Card class="p-4">
              <p class="text-xs text-slate-400">Faucet pool</p>
              <p class="mt-2 text-2xl font-semibold">2.4M ENT</p>
            </Card>
            <Card class="p-4">
              <p class="text-xs text-slate-400">Mock block</p>
              <p class="mt-2 text-2xl font-semibold">420k+</p>
            </Card>
            <Card class="p-4">
              <p class="text-xs text-slate-400">Cooldown</p>
              <p class="mt-2 text-2xl font-semibold">60s</p>
            </Card>
          </div>
        </div>

        <Card class="w-full p-5 sm:p-6">
          <form class="space-y-5" @submit.prevent="requestTokens">
            <div class="flex items-center justify-between gap-4">
              <div>
                <h2 class="text-xl font-semibold">Request ENT</h2>
                <p class="text-sm text-slate-400">Testnet balance updates instantly.</p>
              </div>
              <div class="grid size-11 place-items-center rounded-lg bg-gradient-to-br from-violet-500 to-blue-500">
                <Coins class="size-5" />
              </div>
            </div>

            <label class="block space-y-2" for="address">
              <span class="text-sm font-medium text-slate-200">L2 address</span>
              <Input
                id="address"
                v-model="address"
                placeholder="ent_l2_9f2a4b7c0e..."
              />
            </label>

            <label class="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-white/5 px-3 py-2">
              <span class="text-sm text-slate-300">Simulate queue failure</span>
              <input
                v-model="simulateFailure"
                class="size-4 accent-violet-500"
                type="checkbox"
              />
            </label>

            <Button class="w-full" :disabled="isLoading" size="lg" type="submit">
              <Loader2 v-if="isLoading" class="animate-spin" />
              <Sparkles v-else />
              {{ isLoading ? "Claiming" : `Claim ${FAUCET_AMOUNT} ENT` }}
            </Button>
          </form>

          <div class="mt-5 space-y-3">
            <Alert v-if="status.kind === 'idle'">
              Faucet queue is ready.
            </Alert>

            <Alert v-if="status.kind === 'validation'" variant="error">
              <AlertTriangle class="mr-2 inline size-4" />
              {{ status.message }}
            </Alert>

            <Alert v-if="status.kind === 'network'" variant="warning">
              <Clock3 class="mr-2 inline size-4" />
              Queue retry required.
              <Button class="ml-2" size="sm" variant="outline" @click="retry">
                <RotateCcw />
                Retry
              </Button>
            </Alert>

            <Alert v-if="status.kind === 'cooldown'" variant="warning">
              <Clock3 class="mr-2 inline size-4" />
              Cooldown active. Retry in {{ formatRetry(status.retryAt) }}.
            </Alert>

            <Alert v-if="status.kind === 'success'" variant="success">
              <CheckCircle2 class="mr-2 inline size-4" />
              +{{ status.claim.amount }} ENT sent.
            </Alert>

            <div v-if="currentClaim" class="rounded-lg border border-white/10 bg-slate-900/70 p-4">
              <div class="grid gap-3 sm:grid-cols-2">
                <div>
                  <p class="text-xs text-slate-400">Balance</p>
                  <p class="mt-1 text-2xl font-semibold">{{ currentClaim.balance }} ENT</p>
                </div>
                <div>
                  <p class="text-xs text-slate-400">Block height</p>
                  <p class="mt-1 text-2xl font-semibold">{{ currentClaim.blockHeight }}</p>
                </div>
              </div>
              <div class="mt-4 min-w-0">
                <p class="text-xs text-slate-400">Transaction hash</p>
                <div class="mt-2 flex min-w-0 items-center gap-2">
                  <code class="min-w-0 flex-1 truncate rounded-md bg-black/35 px-2 py-2 text-xs text-blue-100">
                    {{ currentClaim.txHash }}
                  </code>
                  <Button :disabled="!canCopy" size="icon" variant="outline" @click="copyHash">
                    <Copy />
                  </Button>
                </div>
                <p v-if="copied" class="mt-2 text-xs text-emerald-300">Copied</p>
              </div>
            </div>
          </div>
        </Card>
      </section>
    </div>
  </main>
</template>
