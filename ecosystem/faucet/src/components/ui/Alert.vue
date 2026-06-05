<script setup lang="ts">
import { computed } from "vue"

import { cn } from "@/lib/utils"

const props = withDefaults(
  defineProps<{
    class?: string
    variant?: "default" | "success" | "warning" | "error"
  }>(),
  {
    variant: "default",
  },
)

const classes = computed(() =>
  cn(
    "rounded-lg border px-3 py-2 text-sm",
    {
      "border-white/10 bg-white/5 text-slate-200": props.variant === "default",
      "border-emerald-400/30 bg-emerald-400/10 text-emerald-100": props.variant === "success",
      "border-amber-400/30 bg-amber-400/10 text-amber-100": props.variant === "warning",
      "border-rose-400/30 bg-rose-400/10 text-rose-100": props.variant === "error",
    },
    props.class,
  ),
)
</script>

<template>
  <div :class="classes" role="status">
    <slot />
  </div>
</template>
