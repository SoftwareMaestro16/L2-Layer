<script setup lang="ts">
import { cn } from "@/lib/utils"

const model = defineModel<string>({ required: true })

withDefaults(
  defineProps<{
    class?: string
    id?: string
    placeholder?: string
  }>(),
  {
    id: undefined,
    placeholder: undefined,
  },
)
</script>

<template>
  <input
    :id="id"
    v-model="model"
    :class="
      cn(
        'h-11 w-full rounded-lg border border-white/10 bg-slate-950/70 px-3 text-sm text-slate-50 outline-none transition placeholder:text-slate-500 focus:border-violet-400 focus:ring-3 focus:ring-violet-400/20',
        $props.class,
      )
    "
    :placeholder="placeholder"
  />
</template>
