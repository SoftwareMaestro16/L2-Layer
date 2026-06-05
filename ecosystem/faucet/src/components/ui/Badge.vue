<script setup lang="ts">
import { computed } from "vue"

import { cn } from "@/lib/utils"

const props = withDefaults(
  defineProps<{
    class?: string
    variant?: "default" | "blue" | "violet"
  }>(),
  {
    variant: "default",
  },
)

const classes = computed(() =>
  cn(
    "inline-flex items-center rounded-md border px-2 py-1 text-xs font-medium",
    {
      "border-white/10 bg-white/[0.08] text-slate-200": props.variant === "default",
      "border-blue-400/30 bg-blue-400/10 text-blue-100": props.variant === "blue",
      "border-violet-400/30 bg-violet-400/10 text-violet-100": props.variant === "violet",
    },
    props.class,
  ),
)
</script>

<template>
  <span :class="classes">
    <slot />
  </span>
</template>
