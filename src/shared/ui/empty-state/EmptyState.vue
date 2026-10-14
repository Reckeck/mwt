<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { cn } from '@/shared/lib/utils'
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/shared/ui/empty'

import type { EmptyStateProps } from './empty-state.types'

/**
 * The one empty state of the app: what is missing, what will be here, and the
 * button that starts it (default slot). Built on shadcn `Empty`.
 */
const props = withDefaults(defineProps<EmptyStateProps>(), {
  title: undefined,
  description: undefined,
  icon: undefined,
  compact: false,
})

const { t } = useI18n()
</script>

<template>
  <Empty :class="cn('border border-dashed border-border', props.compact ? 'gap-3 p-4 md:p-6' : 'p-6 md:p-8')">
    <EmptyHeader>
      <EmptyMedia v-if="props.icon" variant="icon">
        <component :is="props.icon" />
      </EmptyMedia>
      <EmptyTitle class="text-base">{{ props.title ?? t('ui.empty') }}</EmptyTitle>
      <EmptyDescription v-if="props.description">{{ props.description }}</EmptyDescription>
    </EmptyHeader>
    <EmptyContent v-if="$slots.default">
      <div class="flex flex-wrap items-center justify-center gap-2">
        <slot />
      </div>
    </EmptyContent>
  </Empty>
</template>
