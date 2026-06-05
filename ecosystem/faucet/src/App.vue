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
  ShieldCheck,
  Sparkles,
} from "lucide-vue-next"
import { computed, onMounted, ref } from "vue"
import { z } from "zod"

import Alert from "@/components/ui/Alert.vue"
import Badge from "@/components/ui/Badge.vue"
import Button from "@/components/ui/Button.vue"
import Card from "@/components/ui/Card.vue"
import Input from "@/components/ui/Input.vue"
import { claimEnt, fetchStatus, logout, type Claim, type FaucetStatus } from "@/lib/api"

const accountSchema = z
  .string()
  .trim()
  .min(12, "Enter an Entropis L2 account.")
  .max(96, "Account id is too long.")
  .regex(/^[A-Za-z0-9:_-]+$/, "Use account id, 8:raw hash, 0x hash, or EX address.")

const account = ref("")
const status = ref<FaucetStatus | null>(null)
const error = ref<string | null>(null)
const success = ref<string | null>(null)
const loading = ref(false)
const refreshing = ref(false)

const config = computed(() => status.value?.config ?? null)
const session = computed(() => status.value?.session ?? null)
const claims = computed(() => status.value?.claims ?? [])
const authenticated = computed(() => Boolean(session.value?.authenticated))
const latestClaim = computed(() => claims.value[0] ?? null)
const claimDisabled = computed(
  () => loading.value || !authenticated.value || !config.value?.nodeConfigured,
)

onMounted(() => {
  void refreshStatus()
})

async function refreshStatus() {
  refreshing.value = true
  try {
    status.value = await fetchStatus()
  } catch (caught) {
    error.value = labelError(caught)
  } finally {
    refreshing.value = false
  }
}

async function submitClaim() {
  error.value = null
  success.value = null

  const parsed = accountSchema.safeParse(account.value)
  if (!parsed.success) {
    error.value = parsed.error.issues[0]?.message ?? "Invalid account."
    return
  }

  loading.value = true
  try {
    const result = await claimEnt(parsed.data)
    success.value = result.duplicate
      ? "Existing pending claim returned."
      : `${result.claim.amountEnt} ENT claim queued.`
    await refreshStatus()
  } catch (caught) {
    error.value = labelError(caught)
  } finally {
    loading.value = false
  }
}

async function signOut() {
  await logout()
  status.value = await fetchStatus()
}

function signIn() {
  window.location.href = "/api/auth/github/start"
}

function labelError(caught: unknown) {
  const message = caught instanceof Error ? caught.message : "request_failed"
  const labels: Record<string, string> = {
    github_session_required: "Sign in with GitHub first.",
    github_oauth_not_configured: "GitHub OAuth is not configured.",
    node_admin_not_configured: "Faucet node admin token is not configured.",
    invalid_l2_address: "Invalid Entropis L2 address.",
    invalid_l2_address_checksum: "Invalid EX address checksum.",
    reserved_zero_address: "Zero address is reserved.",
    cooldown: "Cooldown is active for this account or GitHub user.",
    rate_limited: "Rate limit reached. Try again later.",
    account_id_required: "Account id is required.",
  }

  return labels[message] ?? "Faucet request failed."
}

function formatTime(value: number) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(value)
}

function statusVariant(claim: Claim) {
  if (claim.status === "granted" || claim.status === "duplicate") return "green"
  if (claim.status === "failed") return "default"
  return "blue"
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
            <p class="truncate text-xs text-slate-400">GitHub-gated testnet ENT grants</p>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <Badge :variant="config?.githubConfigured ? 'green' : 'default'">
            <Github />
            OAuth
          </Badge>
          <Badge :variant="config?.nodeConfigured ? 'green' : 'default'">
            <ShieldCheck />
            Node
          </Badge>
        </div>
      </header>

      <section class="grid flex-1 items-center gap-6 py-8 lg:grid-cols-[1.02fr_0.98fr] lg:py-10">
        <div class="space-y-5">
          <div class="max-w-2xl space-y-4">
            <div class="flex flex-wrap gap-2">
              <Badge variant="blue">3002 service</Badge>
              <Badge>{{ config?.amountEnt ?? 100 }} ENT per claim</Badge>
              <Badge>{{ status?.pendingCount ?? 0 }} pending</Badge>
            </div>
            <h1 class="text-4xl font-semibold leading-tight text-white sm:text-5xl">
              Claim test ENT with GitHub abuse protection.
            </h1>
            <p class="max-w-xl text-sm leading-6 text-slate-300 sm:text-base">
              Sign in, submit an Entropis L2 account, and track the grant queue without exposing admin credentials.
            </p>
          </div>

          <div class="grid gap-3 sm:grid-cols-3">
            <Card class="p-4">
              <p class="text-xs text-slate-400">Cooldown</p>
              <p class="mt-2 text-2xl font-semibold">
                {{ config?.enforceCooldown ? `${config.cooldownSeconds}s` : "off" }}
              </p>
            </Card>
            <Card class="p-4">
              <p class="text-xs text-slate-400">Batch size</p>
              <p class="mt-2 text-2xl font-semibold">{{ config?.maxBatchSize ?? 100 }}</p>
            </Card>
            <Card class="p-4">
              <p class="text-xs text-slate-400">Interval</p>
              <p class="mt-2 text-2xl font-semibold">
                {{ Math.round((config?.batchIntervalMs ?? 10000) / 1000) }}s
              </p>
            </Card>
          </div>
        </div>

        <Card class="w-full p-5 sm:p-6">
          <div class="space-y-5">
            <div class="flex items-center justify-between gap-4">
              <div>
                <h2 class="text-xl font-semibold">Request ENT</h2>
                <p class="text-sm text-slate-400">
                  {{ authenticated ? `Signed in as ${session?.user?.login}` : "GitHub session required" }}
                </p>
              </div>
              <div class="grid size-11 place-items-center rounded-lg bg-gradient-to-br from-violet-500 to-blue-500">
                <Coins class="size-5" />
              </div>
            </div>

            <div v-if="!authenticated" class="space-y-3">
              <Alert :variant="config?.githubConfigured ? 'default' : 'warning'">
                <AlertTriangle v-if="!config?.githubConfigured" class="mr-2 inline size-4" />
                {{ config?.githubConfigured ? "GitHub identity is required." : "GitHub OAuth is not configured." }}
              </Alert>
              <Button class="w-full" :disabled="!config?.githubConfigured" size="lg" @click="signIn">
                <Github />
                Sign in with GitHub
              </Button>
            </div>

            <form v-else class="space-y-4" @submit.prevent="submitClaim">
              <label class="block space-y-2" for="account">
                <span class="text-sm font-medium text-slate-200">L2 account</span>
                <Input id="account" v-model="account" placeholder="8:1111... or 0x1111..." />
              </label>

              <div class="flex gap-2">
                <Button class="flex-1" :disabled="claimDisabled" size="lg" type="submit">
                  <Loader2 v-if="loading" class="animate-spin" />
                  <Sparkles v-else />
                  {{ loading ? "Queueing" : `Claim ${config?.amountEnt ?? 100} ENT` }}
                </Button>
                <Button size="icon" type="button" variant="outline" @click="refreshStatus">
                  <RefreshCcw :class="{ 'animate-spin': refreshing }" />
                </Button>
                <Button size="icon" type="button" variant="outline" @click="signOut">
                  <LogOut />
                </Button>
              </div>
            </form>

            <Alert v-if="error" variant="error">
              <AlertTriangle class="mr-2 inline size-4" />
              {{ error }}
            </Alert>
            <Alert v-if="success" variant="success">
              <CheckCircle2 class="mr-2 inline size-4" />
              {{ success }}
            </Alert>
            <Alert v-if="authenticated && !config?.nodeConfigured" variant="warning">
              <Clock3 class="mr-2 inline size-4" />
              Node admin integration is not configured.
            </Alert>

            <div v-if="latestClaim" class="rounded-lg border border-white/10 bg-slate-900/70 p-4">
              <div class="flex items-center justify-between gap-3">
                <div>
                  <p class="text-xs text-slate-400">Latest claim</p>
                  <p class="mt-1 text-lg font-semibold">{{ latestClaim.amountEnt }} ENT</p>
                </div>
                <Badge :variant="statusVariant(latestClaim)">
                  {{ latestClaim.status }}
                </Badge>
              </div>
              <code class="mt-3 block truncate rounded-md bg-black/35 px-2 py-2 text-xs text-blue-100">
                {{ latestClaim.accountRawAddress }}
              </code>
              <p class="mt-2 text-xs text-slate-400">
                {{ formatTime(latestClaim.updatedAt) }}
                <span v-if="latestClaim.nodeDepositId">/ {{ latestClaim.nodeDepositId }}</span>
              </p>
            </div>

            <div v-if="claims.length > 1" class="space-y-2">
              <div
                v-for="claim in claims.slice(1, 5)"
                :key="claim.claimId"
                class="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-white/5 px-3 py-2"
              >
                <span class="truncate text-xs text-slate-300">{{ claim.accountRawAddress }}</span>
                <Badge :variant="statusVariant(claim)">{{ claim.status }}</Badge>
              </div>
            </div>
          </div>
        </Card>
      </section>
    </div>
  </main>
</template>
