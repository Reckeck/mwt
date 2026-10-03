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

function onInput(e: Event) {
  model.value = (e.target as HTMLTextAreaElement).value
}
</script>

<template>
  <textarea
    :value="model"
    rows="2"
    :class="cn(
      'w-full resize-y rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
      cls,
    )"
    v-bind="rest"
    @input="onInput"
  />
</template>
