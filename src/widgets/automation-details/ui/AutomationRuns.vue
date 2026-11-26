<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { MoneyPhrase } from '@/entities/currency'
import { AutomationRunActions } from '@/features/automation-edit'
import { formatDayMonthShort } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import { runMoney, runToneClass } from '../lib/card-view'
import { runNeedsAction } from '../lib/run-records'
import { useAutomationRunRecords } from '../model/use-automation-run-records'

/** `Last runs`: the day, what it did, how it ended (Auto-1 · 3). */
const props = defineProps<{
  runs: JournalEntry[]
  names: { store: (id: string) => string; category: (key: string) => string }
  /** The repeat whose runs these are (its failed runs can be retried or skipped); `null` for a scheme. */
  automationId: string | null
  name: string
}>()
const records = useAutomationRunRecords(() => props.automationId)
/** Each run with the record that still waits for the user, if any. */
const rows = computed(() =>
  props.runs.map((run) => {
    const record = records.value.get(run.id)
    return { run, waiting: runNeedsAction(record) ? record : null }
  }),
)

const { t } = useI18n()
</script>

<template>
  <section class="flex flex-col gap-2" data-automation-runs>
    <h3 class="text-sm font-semibold">{{ t('automationCard.runs') }}</h3>
    <p v-if="!runs.length" class="text-sm text-muted-foreground">{{ t('automationCard.noRuns') }}</p>
    <div v-for="{ run, waiting } in rows" :key="run.id" class="flex items-center gap-2 text-sm" data-automation-run>
      <span class="w-[110px] shrink-0 text-muted-foreground tabular-nums">{{ formatDayMonthShort(run.at) }}</span>
      <span class="min-w-0 flex-1 truncate tabular-nums">
        <MoneyPhrase :parts="runMoney(run, names)" />
      </span>
      <AutomationRunActions v-if="waiting" :run="waiting" :name="props.name" />
      <span v-else :class="cn('shrink-0', runToneClass(run.status))">{{ t(`automations.status.${run.status}`) }}</span>
    </div>
  </section>
</template>
