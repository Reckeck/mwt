<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Badge } from '@/shared/ui/badge'
import { scheduleText } from '@/shared/lib/planning'

import type { AutomationSummaryProps } from './automation-summary.types'

/** An automation's card head: its name and state, when it runs and what it books. */
const props = defineProps<AutomationSummaryProps>()
const { t } = useI18n()
</script>

<template>
  <div class="grid min-w-0 gap-1">
    <div class="flex flex-wrap items-center gap-2">
      <p class="truncate font-medium">{{ props.automation.name }}</p>
      <Badge v-if="props.automation.preset" variant="secondary">{{ t(`preset.${props.automation.preset}`) }}</Badge>
      <Badge v-if="props.automation.amount_mode === 'varies'" variant="outline">{{ t('automation.varies.badge') }}</Badge>
      <Badge v-if="!props.automation.active" variant="outline">{{ t('automation.paused') }}</Badge>
      <Badge v-if="props.rejected" variant="destructive">{{ t('automation.pendingRuns', { count: props.rejected }) }}</Badge>
      <Badge v-if="props.unconfirmed" variant="outline" class="text-warning" data-tour="automation-unconfirmed">
        {{ t('automation.unconfirmedRuns', { count: props.unconfirmed }) }}
      </Badge>
    </div>
    <p class="text-sm text-muted-foreground">
      {{ scheduleText(props.automation.schedule, t) }}
      <template v-if="props.automation.workday_shift !== 'none'">
        · {{ t(`automation.workday.short.${props.automation.workday_shift}`) }}
      </template>
    </p>
    <p class="text-sm tabular-nums">{{ props.line }}</p>
  </div>
</template>
