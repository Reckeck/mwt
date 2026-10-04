<script setup lang="ts">
import { computed, useAttrs } from 'vue'

import { cn } from '@/lib/utils'

defineOptions({ inheritAttrs: false })
const model = defineModel<string>({ default: '' })

const attrs = useAttrs()
const cls = computed(() => (attrs as Record<string, unknown>).class as string | undefined)
const rest = computed(() => {
  const { class: _cls, ...others } = attrs as Record<string, unknown>
  return others
})

function onChange(event: Event) {
  model.value = (event.target as HTMLSelectElement).value
}
</script>

<template>
  <select
    :value="model"
    :class="cn(
      'h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
      cls,
    )"
    v-bind="rest"
    @change="onChange"
  >
    <slot />
  </select>
</template>
