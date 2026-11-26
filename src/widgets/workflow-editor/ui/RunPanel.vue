<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ClipboardCheck, X } from 'lucide-vue-next'
import { computed } from 'vue'

import { MoneyPhrase } from '@/entities/currency'
import { OutcomeBadge, explainRun } from '@/entities/workflow'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { EntityLink } from '@/shared/ui/entity-link'
import { formatDateTime, formatDayMonthShort } from '@/shared/lib/format'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'
import { call } from '@/shared/api/ipc'
import { useQuery } from '@tanstack/vue-query'

import { useRunSummary } from '../model/use-run-summary'

/**
 * The last emulation or run under the canvas (Auto-3 · 15): what it asks,
 * answered right here, what it would make, and that nothing was written.
 * The run order itself is the numbers on the nodes.
 */
const props = defineProps<{ report: RunReport; canSaveTest?: boolean; answers?: Record<string, boolean> }>()
const emit = defineEmits<{ focus: [nodeId: string]; close: []; saveTest: []; answer: [nodeId: string, yes: boolean] }>()

const { t, locale } = useI18n()
const summary = useRunSummary(
  () => props.report,
  () => props.answers,
)
const question = summary.question
const dry = computed(() => props.report.run.dry_run)
const reply = (yes: boolean) => question.value && emit('answer', question.value.nodeId, yes)

/** The workflows an emulated event would start, by name. */
const workflows = useQuery({ queryKey: WORKFLOW_KEYS.list, queryFn: () => call<Workflow[]>('list_workflows') })
const nameOf = (id: string) => (workflows.data.value ?? []).find((entry) => entry.id === id)?.name ?? id
</script>

<template>
  <section class="grid max-h-80 shrink-0 gap-2 overflow-y-auto rounded-lg border border-border bg-card px-4 py-3" data-tour="workflow-run-panel">
    <header class="flex items-center gap-2">
      <Badge variant="secondary" class="shrink-0">{{ dry ? t('workflow.runPanel.dry') : t('workflow.runPanel.live') }}</Badge>
      <h3 v-if="question" class="min-w-0 flex-1 truncate text-sm font-semibold">{{ t('workflow.runPanel.askTitle') }}</h3>
      <span v-else class="flex-1"><OutcomeBadge :outcome="props.report.run.outcome" /></span>
      <span class="shrink-0 text-xs text-muted-foreground tabular-nums">{{ formatDayMonthShort(props.report.run.started_at, ' ') }}</span>
      <Button variant="ghost" size="icon" :aria-label="t('actions.close')" @click="emit('close')"><X class="size-4" /></Button>
    </header>
    <div v-if="question" class="flex flex-wrap items-center gap-3" data-tour="workflow-emulation-question">
      <Button variant="link" size="block" class="min-w-0 flex-1 p-0 text-left text-sm font-normal text-foreground" @click="emit('focus', question.nodeId)">
        «{{ question.question || t('workflow.defaultQuestion') }}»
      </Button>
      <div class="flex shrink-0 gap-2">
        <Button size="sm" variant="outline" data-run-answer="no" @click="reply(false)">{{ t('workflow.runPanel.no') }}</Button>
        <Button size="sm" data-run-answer="yes" @click="reply(true)">{{ t('workflow.runPanel.yes') }}</Button>
      </div>
    </div>
    <p v-else class="text-sm" data-tour="workflow-explain">{{ explainRun(props.report, t, locale) }}</p>
    <p v-if="summary.made.value.length || question" class="text-xs text-muted-foreground">
      <MoneyPhrase :parts="summary.made.value" />
      <template v-if="question">{{ t('workflow.runPanel.questionHint') }}</template>
    </p>
    <p v-if="dry && summary.answered.value" class="text-xs text-muted-foreground">
      {{ t('workflow.runPanel.answers', { list: summary.answered.value }) }}
    </p>
    <div v-if="props.report.queued.length" class="grid gap-1">
      <h4 class="text-xs font-medium text-muted-foreground">{{ t('workflow.runPanel.queued') }}</h4>
      <p v-for="batch in props.report.queued" :key="batch.id" class="text-sm">{{ batch.summary }}</p>
    </div>
    <div v-if="props.report.notices.length" class="grid gap-1">
      <h4 class="text-xs font-medium text-muted-foreground">{{ t('workflow.runPanel.notices') }}</h4>
      <p v-for="notice in props.report.notices" :key="notice.key" class="text-sm">{{ notice.title }}: {{ notice.body }}</p>
    </div>
    <div v-if="props.report.event_runs.length" class="grid gap-1" data-tour="workflow-event-runs">
      <h4 class="text-xs font-medium text-muted-foreground">{{ t('workflow.runPanel.eventRuns') }}</h4>
      <p v-for="run in props.report.event_runs" :key="run.id" class="flex flex-wrap items-center gap-2 text-sm">
        <EntityLink :target="{ kind: 'workflow', id: run.workflow_id }" :label="nameOf(run.workflow_id)" plain />
        <OutcomeBadge :outcome="run.outcome" />
        <span class="text-xs text-muted-foreground">{{ t('workflow.runPanel.eventSteps', { count: run.trace.length }) }}</span>
      </p>
    </div>
    <div v-if="props.report.reminders.length" class="grid gap-1">
      <h4 class="text-xs font-medium text-muted-foreground">{{ dry ? t('workflow.runPanel.wouldRemind') : t('workflow.runPanel.reminders') }}</h4>
      <p v-for="reminder in props.report.reminders" :key="reminder.id" class="text-sm">{{ formatDateTime(reminder.due_at) }} · {{ reminder.text }}</p>
    </div>
    <footer v-if="props.canSaveTest || dry" class="flex flex-wrap items-center gap-3">
      <Button v-if="props.canSaveTest" size="sm" variant="outline" data-tour="workflow-save-test" @click="emit('saveTest')">
        <ClipboardCheck class="size-4" />
        {{ t('workflow.tests.saveFromRun') }}
      </Button>
      <span v-if="dry" class="text-xs text-muted-foreground">{{ t('workflow.runPanel.nothingWritten') }}</span>
    </footer>
  </section>
</template>
