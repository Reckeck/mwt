<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Check, ClipboardCheck, MessageCircleQuestion, X } from 'lucide-vue-next'
import { computed } from 'vue'

import { OutcomeBadge, RunTrace, explainRun } from '@/entities/workflow'
import { Button } from '@/shared/ui/button'
import { formatDateTime, formatMoney } from '@/shared/lib/format'
import { txId, txKind, txLegs } from '@/shared/lib/transactions'
import { WORKFLOW_KEYS, outcomeQuestion } from '@/shared/lib/workflow'
import { call } from '@/shared/api/ipc'
import { useQuery } from '@tanstack/vue-query'

/** The last emulation or run: the steps in words and what it would make. */
const props = defineProps<{ report: RunReport; canSaveTest?: boolean; answers?: Record<string, boolean> }>()
const emit = defineEmits<{ focus: [nodeId: string]; close: []; saveTest: []; answer: [nodeId: string, yes: boolean] }>()

const { t, locale } = useI18n()

/** An emulation that stopped at «Ждать подтверждения»: answered right here. */
const question = computed(() => (props.report.run.dry_run ? outcomeQuestion(props.report.run.outcome) : null))
const answered = computed(() =>
  Object.values(props.answers ?? {})
    .map((yes) => t(yes ? 'workflow.runPanel.yes' : 'workflow.runPanel.no'))
    .join(', '),
)

/** The workflows an emulated event would start, by name. */
const workflows = useQuery({ queryKey: WORKFLOW_KEYS.list, queryFn: () => call<Workflow[]>('list_workflows') })
const nameOf = (id: string) => (workflows.data.value ?? []).find((entry) => entry.id === id)?.name ?? id

function line(tx: Transaction): string {
  const leg = txLegs(tx).find((entry) => Number(entry.amount) < 0) ?? txLegs(tx)[0]
  const amount = leg ? formatMoney(String(Math.abs(Number(leg.amount))), { currency: leg.asset.symbol }) : ''
  return `${t(`tx.kind.${txKind(tx)}`)} · ${amount}`
}
</script>

<template>
  <section class="grid max-h-80 gap-3 overflow-y-auto rounded-lg border border-border bg-card p-3" data-tour="workflow-run-panel">
    <header class="flex items-center gap-2">
      <h3 class="text-sm font-semibold">
        {{ props.report.run.dry_run ? t('workflow.runPanel.dry') : t('workflow.runPanel.live') }}
      </h3>
      <OutcomeBadge :outcome="props.report.run.outcome" />
      <span class="flex-1" />
      <Button v-if="props.canSaveTest" size="sm" variant="outline" data-tour="workflow-save-test" @click="emit('saveTest')">
        <ClipboardCheck class="size-4" />
        {{ t('workflow.tests.saveFromRun') }}
      </Button>
      <Button variant="ghost" size="icon" :aria-label="t('actions.close')" @click="emit('close')"><X class="size-4" /></Button>
    </header>
    <p class="text-sm" data-tour="workflow-explain">{{ explainRun(props.report, t, locale) }}</p>
    <div v-if="question" class="grid gap-2 rounded-md border border-primary/40 bg-primary/5 p-3" data-tour="workflow-emulation-question">
      <div class="flex items-start gap-2">
        <MessageCircleQuestion class="mt-0.5 size-4 shrink-0 text-primary" />
        <div class="grid gap-1">
          <span class="text-xs font-medium uppercase text-muted-foreground">{{ t('workflow.runPanel.question') }}</span>
          <button type="button" class="text-left font-medium hover:underline" @click="emit('focus', question.nodeId)">{{ question.question }}</button>
        </div>
      </div>
      <p class="text-xs text-muted-foreground">{{ t('workflow.runPanel.questionHint') }}</p>
      <div class="flex gap-2">
        <Button size="sm" @click="question && emit('answer', question.nodeId, true)">
          <Check class="size-4" />
          {{ t('workflow.runPanel.yes') }}
        </Button>
        <Button size="sm" variant="outline" @click="question && emit('answer', question.nodeId, false)">
          <X class="size-4" />
          {{ t('workflow.runPanel.no') }}
        </Button>
      </div>
    </div>
    <p v-if="props.report.run.dry_run && answered" class="text-xs text-muted-foreground">
      {{ t('workflow.runPanel.answers', { list: answered }) }}
    </p>
    <RunTrace :steps="props.report.run.trace" @focus="(id) => emit('focus', id)" />
    <div v-if="props.report.transactions.length" class="grid gap-1">
      <h4 class="text-xs font-medium uppercase text-muted-foreground">
        {{ props.report.run.dry_run ? t('workflow.runPanel.wouldMake') : t('workflow.runPanel.made') }}
      </h4>
      <p v-for="tx in props.report.transactions" :key="txId(tx)" class="text-sm">{{ line(tx) }}</p>
    </div>
    <div v-if="props.report.queued.length" class="grid gap-1">
      <h4 class="text-xs font-medium uppercase text-muted-foreground">{{ t('workflow.runPanel.queued') }}</h4>
      <p v-for="batch in props.report.queued" :key="batch.id" class="text-sm">{{ batch.summary }}</p>
    </div>
    <div v-if="props.report.notices.length" class="grid gap-1">
      <h4 class="text-xs font-medium uppercase text-muted-foreground">{{ t('workflow.runPanel.notices') }}</h4>
      <p v-for="notice in props.report.notices" :key="notice.key" class="text-sm">{{ notice.title }}: {{ notice.body }}</p>
    </div>
    <div v-if="props.report.event_runs.length" class="grid gap-1" data-tour="workflow-event-runs">
      <h4 class="text-xs font-medium uppercase text-muted-foreground">{{ t('workflow.runPanel.eventRuns') }}</h4>
      <p v-for="run in props.report.event_runs" :key="run.id" class="flex flex-wrap items-center gap-2 text-sm">
        <span>{{ nameOf(run.workflow_id) }}</span>
        <OutcomeBadge :outcome="run.outcome" />
        <span class="text-xs text-muted-foreground">{{ t('workflow.runPanel.eventSteps', { count: run.trace.length }) }}</span>
      </p>
    </div>
    <div v-if="props.report.reminders.length" class="grid gap-1">
      <h4 class="text-xs font-medium uppercase text-muted-foreground">
        {{ props.report.run.dry_run ? t('workflow.runPanel.wouldRemind') : t('workflow.runPanel.reminders') }}
      </h4>
      <p v-for="reminder in props.report.reminders" :key="reminder.id" class="text-sm">
        {{ formatDateTime(reminder.due_at) }} · {{ reminder.text }}
      </p>
    </div>
  </section>
</template>
