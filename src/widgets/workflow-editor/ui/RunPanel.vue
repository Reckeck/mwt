<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ClipboardCheck, X } from 'lucide-vue-next'

import { OutcomeBadge, RunTrace, explainRun } from '@/entities/workflow'
import { Button } from '@/shared/ui/button'
import { formatMoney } from '@/shared/lib/format'
import { txId, txKind, txLegs } from '@/shared/lib/transactions'

/** The last emulation or run: the steps in words and what it would make. */
const props = defineProps<{ report: RunReport; canSaveTest?: boolean }>()
const emit = defineEmits<{ focus: [nodeId: string]; close: []; saveTest: [] }>()

const { t, locale } = useI18n()

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
  </section>
</template>
