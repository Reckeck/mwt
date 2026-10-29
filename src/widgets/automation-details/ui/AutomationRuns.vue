<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { MoneyPhrase } from '@/entities/currency'
import { formatDayMonthShort } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import { type RunTone, runMoney, statusTone } from '../lib/card-view'

/** `Last runs`: the day, what it did, how it ended (Auto-1 · 3). */
defineProps<{
  runs: JournalEntry[]
  names: { store: (id: string) => string; category: (key: string) => string }
}>()

const { t } = useI18n()
const TONE: Record<RunTone, string> = {
  pos: 'text-success',
  wrn: 'text-warning',
  neg: 'text-destructive',
  mut: 'text-muted-foreground',
}
</script>

<template>
  <section class="flex flex-col gap-2" data-automation-runs>
    <h3 class="text-sm font-semibold">{{ t('automationCard.runs') }}</h3>
    <p v-if="!runs.length" class="text-sm text-muted-foreground">{{ t('automationCard.noRuns') }}</p>
    <div v-for="run in runs" :key="run.id" class="flex items-center gap-2 text-sm" data-automation-run>
      <span class="w-[110px] shrink-0 text-muted-foreground tabular-nums">{{ formatDayMonthShort(run.at) }}</span>
      <span class="min-w-0 flex-1 truncate tabular-nums">
        <MoneyPhrase :parts="runMoney(run, names)" />
      </span>
      <span :class="cn('shrink-0', TONE[statusTone(run.status)])">{{ t(`automations.status.${run.status}`) }}</span>
    </div>
  </section>
</template>
