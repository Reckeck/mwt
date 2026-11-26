<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { MoneyPhrase } from '@/entities/currency'
import { OutcomeBadge, explainRun } from '@/entities/workflow'
import { Button } from '@/shared/ui/button'

import { useRunSummary } from '../model/use-run-summary'

/**
 * The phone's emulation under the steps (Auto-3 · 15): the question with
 * `No` / `Yes` in reach of the thumb; any other run in a line, the full
 * report in the sheet behind `Details`.
 */
const props = defineProps<{ report: RunReport; answers?: Record<string, boolean> }>()
const emit = defineEmits<{ answer: [nodeId: string, yes: boolean]; details: [] }>()

const { t, locale } = useI18n()
const summary = useRunSummary(
  () => props.report,
  () => props.answers,
)
const question = summary.question
const reply = (yes: boolean) => question.value && emit('answer', question.value.nodeId, yes)
</script>

<template>
  <section v-if="question" class="grid gap-3 rounded-lg border border-primary p-4" data-tour="workflow-emulation-question" data-run-card>
    <h3 class="text-sm font-semibold">{{ t('workflow.runPanel.askShort') }}</h3>
    <p class="text-sm">«{{ question.question || t('workflow.defaultQuestion') }}»</p>
    <div class="grid grid-cols-2 gap-3">
      <Button variant="outline" class="h-11" data-run-answer="no" @click="reply(false)">{{ t('workflow.runPanel.no') }}</Button>
      <Button class="h-11" data-run-answer="yes" @click="reply(true)">{{ t('workflow.runPanel.yes') }}</Button>
    </div>
  </section>
  <section v-else class="grid gap-2 rounded-lg border border-border p-4" data-run-card>
    <div class="flex items-center gap-2">
      <h3 class="text-sm font-semibold">{{ props.report.run.dry_run ? t('workflow.runPanel.dry') : t('workflow.runPanel.live') }}</h3>
      <OutcomeBadge :outcome="props.report.run.outcome" />
    </div>
    <p class="text-sm text-muted-foreground">
      <template v-if="summary.made.value.length">
        <MoneyPhrase :parts="summary.made.value" />
      </template>
      <template v-else>{{ explainRun(props.report, t, locale) }}</template>
    </p>
    <Button variant="outline" class="h-11" @click="emit('details')">{{ t('workflow.runPanel.details') }}</Button>
  </section>
</template>
