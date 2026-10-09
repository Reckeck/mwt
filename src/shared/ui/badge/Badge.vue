<script setup lang="ts">
import { computed, useAttrs } from 'vue'

import { cn } from '@/shared/lib/utils'

import { badgeVariants, type BadgeProps } from './badge.variants'

defineOptions({ inheritAttrs: false })


withDefaults(
  defineProps<BadgeProps>(),
  { variant: 'default' },
)

const attrs = useAttrs()
const cls = computed(() => (attrs as Record<string, unknown>).class as string | undefined)
const rest = computed(() => {
  const { class: _cls, ...others } = attrs as Record<string, unknown>
  return others
})
</script>

<template>
  <span :class="cn(badgeVariants({ variant }), cls)" v-bind="rest">
    <slot />
  </span>
</template>