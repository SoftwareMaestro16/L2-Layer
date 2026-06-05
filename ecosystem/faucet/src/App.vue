<script setup lang="ts">
import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Coins,
  Github,
  Loader2,
  LogOut,
  RefreshCcw,
  Send,
} from "lucide-vue-next"
import { computed, onMounted, ref } from "vue"

import Alert from "@/components/ui/Alert.vue"
import Badge from "@/components/ui/Badge.vue"
import Button from "@/components/ui/Button.vue"
import Card from "@/components/ui/Card.vue"
import Input from "@/components/ui/Input.vue"
import {
  addressSchema,
  FAUCET_AMOUNT,
  fetchFaucetBatches,
  fetchFaucetStatus,
  logoutFaucet,
  submitFaucetClaim,
  type FaucetBatch,
  type FaucetClaim,
  type FaucetStatus,
} from "@/lib/faucet"

type ViewStatus =
  | { kind: "idle" }
  | { kind: "success"; message: string }
  | { kind: "error"; message: string }

const address = ref("")
const isLoading = ref(false)
const status = ref<FaucetStatus | null>(null)
const batches = ref<FaucetBatch[]>([])
const viewStatus = ref<ViewStatus>({ kind: "idle" })

const authenticated = computed(() => status.value?.session.authenticated ?? false)
const user = computed(() => status.value?.session.user ?? null)
const config = computed(() => status.value?.config)
const claims = computed<FaucetClaim[]>(() => status.value?.claims ?? [])

onMounted(() => {
  void refresh()
})

async function refresh() {
  const [nextStatus, nextBatches] = await Promise.all([fetchFaucetStatus(), fetchFaucetBatches()])
  status.value = nextStatus
  batches.value = nextBatches.batches
}

async function requestClaim() {
  const parsed = addressSchema.safeParse(address.value)
  if (!parsed.success) {
    viewStatus.value = { kind: "error", message: parsed.error.issues[0]?.message ?? "Invalid address." }
    return
  }

  isLoading.value = true
  viewStatus.value = { kind: "idle" }
  try {
    const result = await submitFaucetClaim(parsed.data)
    viewStatus.value = {
      kind: "success",
      message: result.duplicate ? "Claim is already queued." : "Claim queued for the next batch.",
    }
    address.value = ""
    await refresh()
  } catch (error) {
    viewStatus.value = { kind: "error", message: error instanceof Error ? error.message : "claim_failed" }
  } finally {
    isLoading.value = false
  }
}

async function logout() {
  await logoutFaucet()
  await refresh()
}

function login() {
  window.location.href = "/api/auth/github/start"
}

function formatTime(timestamp: number | null) {
  if (!timestamp) return "pending"
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(timestamp))
}
</script>

<template>
  <main class="min-h-screen bg-slate-950 text-slate-50">
    <div class="absolute inset-0 -z-10 faucet-bg" />
    <div class="mx-auto flex min-h-screen w-full max-w-6xl flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8">
      <header class="flex flex-wrap items-center justify-between gap-3">
        <div class="flex min-w-0 items-center gap-3">
          <img
            src="/entropis.png"
            alt="Entropis"
            class="size-10 rounded-lg border border-white/10 bg-slate-900 object-cover"
          />
          <div class="min-w-0">
            <p class="text-sm font-semibold text-slate-100">Entropis Faucet</p>
            <p class="truncate text-xs text-slate-400">GitHub-gated testnet ENT</p>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <Badge :variant="config?.nodeConfigured ? 'blue' : 'violet'">
            <Coins />
            {{ config?.amountEnt ?? FAUCET_AMOUNT }} ENT
          </Badge>
          <Button v-if="authenticated" variant="outline" @click="logout">
            <LogOut />
            Logout
          </Button>
          <Button v-else @click="login">
            <Github />
            GitHub
          </Button>
        </div>
      </header>

      <section class="grid gap-4 md:grid-cols-4">
        <Card class="p-4">
          <p class="text-xs text-slate-400">Session</p>
          <p class="mt-2 truncate text-lg font-semibold">{{ user?.login ?? "not signed in" }}</p>
        </Card>
        <Card class="p-4">
          <p class="text-xs text-slate-400">Pending queue</p>
          <p class="mt-2 text-lg font-semibold">{{ status?.pendingCount ?? 0 }}</p>
        </Card>
        <Card class="p-4">
          <p class="text-xs text-slate-400">Batch interval</p>
          <p class="mt-2 text-lg font-semibold">{{ (config?.batchIntervalMs ?? 10000) / 1000 }}s</p>
        </Card>
        <Card class="p-4">
          <p class="text-xs text-slate-400">Cooldown</p>
          <p class="mt-2 text-lg font-semibold">{{ config?.enforceCooldown ? "2h" : "test off" }}</p>
        </Card>
      </section>

      <section class="grid flex-1 gap-4 lg:grid-cols-[0.85fr_1.15fr]">
        <Card class="p-5">
          <form class="space-y-4" @submit.prevent="requestClaim">
            <div class="flex items-center justify-between gap-3">
              <div>
                <h1 class="text-xl font-semibold">Request ENT</h1>
                <p class="text-sm text-slate-400">Batch faucet queue</p>
              </div>
              <Button size="icon" type="button" variant="outline" @click="refresh">
                <RefreshCcw />
              </Button>
            </div>

            <Alert v-if="!authenticated" variant="warning">
              <Clock3 class="mr-2 inline size-4" />
              GitHub session required.
            </Alert>
            <Alert v-if="viewStatus.kind === 'success'" variant="success">
              <CheckCircle2 class="mr-2 inline size-4" />
              {{ viewStatus.message }}
            </Alert>
            <Alert v-if="viewStatus.kind === 'error'" variant="error">
              <AlertTriangle class="mr-2 inline size-4" />
              {{ viewStatus.message }}
            </Alert>

            <label class="block space-y-2" for="address">
              <span class="text-sm font-medium text-slate-200">L2 address</span>
              <Input id="address" v-model="address" placeholder="8:... or EX..." />
            </label>

            <Button class="w-full" :disabled="!authenticated || isLoading" size="lg" type="submit">
              <Loader2 v-if="isLoading" class="animate-spin" />
              <Send v-else />
              {{ isLoading ? "Queueing" : `Queue ${config?.amountEnt ?? FAUCET_AMOUNT} ENT` }}
            </Button>
          </form>
        </Card>

        <div class="grid gap-4">
          <Card class="p-5">
            <h2 class="text-lg font-semibold">Recent Claims</h2>
            <div class="mt-4 overflow-hidden rounded-lg border border-white/10">
              <table class="w-full text-left text-sm">
                <thead class="bg-white/5 text-xs text-slate-400">
                  <tr>
                    <th class="px-3 py-2">Address</th>
                    <th class="px-3 py-2">Status</th>
                    <th class="px-3 py-2">Attempts</th>
                    <th class="px-3 py-2">Updated</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-white/10">
                  <tr v-for="claim in claims" :key="claim.claimId">
                    <td class="max-w-[220px] truncate px-3 py-2 font-mono text-xs">{{ claim.accountRawAddress }}</td>
                    <td class="px-3 py-2">{{ claim.status }}</td>
                    <td class="px-3 py-2">{{ claim.attempts }}</td>
                    <td class="px-3 py-2">{{ formatTime(claim.updatedAt) }}</td>
                  </tr>
                  <tr v-if="claims.length === 0">
                    <td class="px-3 py-4 text-slate-400" colspan="4">No claims</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </Card>

          <Card class="p-5">
            <h2 class="text-lg font-semibold">Recent Batches</h2>
            <div class="mt-4 grid gap-2">
              <div
                v-for="batch in batches"
                :key="batch.batchId"
                class="grid gap-2 rounded-lg border border-white/10 bg-slate-900/70 p-3 text-sm sm:grid-cols-[1fr_auto_auto]"
              >
                <code class="truncate text-xs text-blue-100">{{ batch.batchId }}</code>
                <span>{{ batch.status }}</span>
                <span class="text-slate-400">{{ batch.claimIds.length }} claims</span>
              </div>
              <Alert v-if="batches.length === 0">No batches</Alert>
            </div>
          </Card>
        </div>
      </section>
    </div>
  </main>
</template>
