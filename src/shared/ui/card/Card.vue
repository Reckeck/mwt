<script setup lang="ts">
import { computed, useAttrs } from 'vue'

import { cn } from '@/shared/lib/utils'

defineOptions({ inheritAttrs: false })
const attrs = useAttrs()
const cls = computed(() => (attrs as Record<string, unknown>).class as string | undefined)
const rest = computed(() => {
  const { class: _cls, ...others } = attrs as Record<string, unknown>
  return others
})
</script>

<template>
  <div :class="cn('rounded-xl border border-border bg-card text-card-foreground shadow', cls)" v-bind="rest">
    <div v-if="$slots.header" class="flex flex-col gap-y-1.5 p-6 pb-3">
      <slot name="header" />
    </div>
    <div v-if="$slots.default" class="p-6 pt-0">
      <slot />
    </div>
    <div v-if="$slots.footer" class="flex items-center p-6 pt-0">
      <slot name="footer" />
    </div>
  </div>
</template>