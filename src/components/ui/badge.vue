<script setup lang="ts">
import { cva, type VariantProps } from 'class-variance-authority'
import { computed, useAttrs } from 'vue'

import { cn } from '@/lib/utils'

defineOptions({ inheritAttrs: false })

const badgeVariants = cva(
  'inline-flex items-center rounded-md border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2',
  {
    variants: {
      variant: {
        default: 'border-transparent bg-primary text-primary-foreground shadow',
        secondary: 'border-transparent bg-secondary text-secondary-foreground',
        destructive: 'border-transparent bg-destructive text-destructive-foreground shadow',
        outline: 'text-foreground',
      },
    },
    defaultVariants: { variant: 'default' },
  },
)

type BadgeVariants = VariantProps<typeof badgeVariants>

withDefaults(
  defineProps<{
    variant?: BadgeVariants['variant']
  }>(),
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