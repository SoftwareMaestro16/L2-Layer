<script setup lang="ts">
import { computed } from "vue"

import { cn } from "@/lib/utils"

const props = withDefaults(
  defineProps<{
    class?: string
    disabled?: boolean
    size?: "default" | "sm" | "lg" | "icon"
    type?: "button" | "submit" | "reset"
    variant?: "default" | "secondary" | "outline" | "ghost"
  }>(),
  {
    disabled: false,
    size: "default",
    type: "button",
    variant: "default",
  },
)

const classes = computed(() =>
  cn(
    "inline-flex shrink-0 items-center justify-center gap-2 rounded-lg text-sm font-medium transition-all outline-none focus-visible:ring-3 focus-visible:ring-violet-400/40 disabled:pointer-events-none disabled:opacity-50 [&_svg]:size-4 [&_svg]:shrink-0",
    {
      "bg-gradient-to-r from-violet-500 to-blue-500 text-white shadow-lg shadow-violet-950/30 hover:from-violet-400 hover:to-blue-400":
        props.variant === "default",
      "bg-slate-800 text-slate-100 hover:bg-slate-700": props.variant === "secondary",
      "border border-white/12 bg-white/5 text-slate-100 hover:bg-white/10":
        props.variant === "outline",
      "text-slate-200 hover:bg-white/10": props.variant === "ghost",
      "h-9 px-3": props.size === "default",
      "h-8 px-2.5 text-xs": props.size === "sm",
      "h-11 px-4": props.size === "lg",
      "size-9": props.size === "icon",
    },
    props.class,
  ),
)
</script>

<template>
  <button :class="classes" :disabled="disabled" :type="type">
    <slot />
  </button>
</template>
