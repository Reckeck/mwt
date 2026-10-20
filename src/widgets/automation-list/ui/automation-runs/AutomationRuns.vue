<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { AutomationRunActions } from '@/features/automation-edit'
import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { formatDate } from '@/shared/lib/format'
import { rejectText } from '@/shared/lib/planning'

import type { AutomationRunsProps } from './automation-runs.types'

/** An automation's run history, folded: each run's outcome and what it still waits for. */
const props = defineProps<AutomationRunsProps>()
const { t } = useI18n()
const open = ref(false)

function outcomeText(run: AutomationRun): string {
  if (run.outcome === 'skipped') return t('automation.run.outcome.skipped')
  if ('created' in run.outcome)
    return run.estimated ? t('automation.run.outcome.estimated') : t('automation.run.outcome.created')
  return rejectText(run.outcome.rejected.reason, t, props.storeName)
}

/** A refusal to retry or skip, or an approximate amount to put in. */
const waits = (run: AutomationRun) => run.estimated || typeof run.outcome !== 'object' || !('created' in run.outcome)
const refused = (run: AutomationRun) => typeof run.outcome === 'object' && 'rejected' in run.outcome
</script>

<template>
  <Button
    variant="ghost"
    size="block"
    class="flex w-fit items-center gap-1 px-1 py-0.5 text-xs text-muted-foreground hover:text-foreground"
    :aria-expanded="open"
    @click="open = !open"
  >
    <ChevronDown class="size-3.5 transition-transform" :class="open && 'rotate-180'" />
    {{ t('automation.history', { count: props.runs.length }) }}
  </Button>

  <ul v-if="open" class="grid gap-1">
    <li v-if="!props.runs.length" class="text-xs text-muted-foreground">{{ t('automation.noRuns') }}</li>
    <li
      v-for="run in props.runs"
      :key="run.id"
      class="flex flex-wrap items-center justify-between gap-2 rounded-md bg-muted/40 px-3 py-2 text-sm"
    >
      <span class="flex min-w-0 items-center gap-2">
        <span class="shrink-0 tabular-nums text-muted-foreground">{{ formatDate(run.planned_at) }}</span>
        <span class="min-w-0" :class="refused(run) ? 'text-destructive' : run.estimated && 'text-warning'">
          {{ outcomeText(run) }}
        </span>
        <span v-if="run.amount" class="shrink-0 tabular-nums">
          <template v-if="run.estimated">~</template><Money :amount="run.amount" :currency="run.symbol.toLowerCase()" />
        </span>
      </span>
      <AutomationRunActions v-if="waits(run)" :run="run" :name="props.name" />
    </li>
  </ul>
</template>
