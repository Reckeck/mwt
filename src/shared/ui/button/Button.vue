<script setup lang="ts">
import { computed, useAttrs } from 'vue'

import { cn } from '@/shared/lib/utils'

import { buttonVariants, type ButtonProps } from './button.variants'

defineOptions({ inheritAttrs: false })


withDefaults(
  defineProps<ButtonProps>(),
  { variant: 'default', size: 'default' },
)

const attrs = useAttrs()
const cls = computed(() => (attrs as Record<string, unknown>).class as string | undefined)
const rest = computed(() => {
  const { class: _cls, ...others } = attrs as Record<string, unknown>
  return others
})
</script>

<template>
  <!-- `type` defaults to button; callers pass `type="submit"` through $attrs to override. -->
  <button type="button" :class="cn(buttonVariants({ variant, size }), cls)" v-bind="rest">
    <slot />
  </button>
</template>