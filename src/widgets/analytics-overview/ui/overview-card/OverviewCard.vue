<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { ChevronRight } from 'lucide-vue-next'

import { ReportInsights } from '@/entities/report'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'

import type { OverviewCardProps } from './overview-card.types'

/** One question of the overview: its figures, what they mean, and the way to the report. */
const props = defineProps<OverviewCardProps>()

const { t } = useI18n()
</script>

<template>
  <Card class="min-w-0">
    <template #header>
      <div class="flex items-start justify-between gap-2">
        <div class="min-w-0">
          <h2 class="heading-card">{{ props.title }}</h2>
          <p class="text-xs text-muted-foreground">{{ props.period }}</p>
        </div>
        <RouterLink
          :to="props.to"
          class="inline-flex shrink-0 items-center gap-0.5 rounded-md text-sm text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          {{ t('analytics.overview.details') }}
          <ChevronRight class="size-4" />
        </RouterLink>
      </div>
    </template>
    <div v-if="props.hasData" class="grid gap-4">
      <div class="grid grid-cols-2 gap-3 @mid:grid-cols-3"><slot /></div>
      <div data-tour="overview-insights"><ReportInsights :insights="props.insights" :currency="props.currency" /></div>
    </div>
    <EmptyState v-else compact :title="props.emptyTitle" :description="props.emptyDescription">
      <slot name="action" />
    </EmptyState>
  </Card>
</template>
