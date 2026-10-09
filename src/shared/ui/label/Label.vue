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
  <!-- biome-ignore lint/a11y/noLabelWithoutControl: primitive forwards `for` from $attrs to the caller. -->
  <label
    :class="cn('text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70', cls)"
    v-bind="rest"
  >
    <slot />
  </label>
</template>