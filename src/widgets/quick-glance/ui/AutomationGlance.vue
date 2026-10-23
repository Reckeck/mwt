<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { AutomationAmountDialog } from '@/features/automation-edit'
import { useRoute, useRouter } from 'vue-router'

import { entityLocation } from '@/shared/lib/entity-link'
import { formatDateTime, formatDayMonthShort, formatMoney } from '@/shared/lib/format'
import { scheduleText } from '@/shared/lib/planning'
import { txId, txPrincipalLegs } from '@/shared/lib/transactions'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Glance } from '@/shared/ui/glance'

import { useAutomationGlance } from '../model/use-automation-glance'
import type { AutomationGlanceProps } from '../quick-glance.types'

/**
 * «авто» in an operation row (Paths 2 · 10): which automation booked it, by
 * what rule and when it runs next; «Изменить сумму» corrects this very record,
 * «Журнал →» and «Автоматизация →» lead on. A workflow's record says so too.
 */
const props = defineProps<AutomationGlanceProps>()
const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const open = ref(false)
const amountOpen = ref(false)
const glance = useAutomationGlance(() => props.automationId, () => props.runId, open)

const automation = computed(() => glance.automation.value)
const rule = computed(() => {
  const item = automation.value
  if (!item) return ''
  const shift = item.workday_shift === 'none' ? '' : `; ${t(`automation.workday.short.${item.workday_shift}`)}`
  return scheduleText(item.schedule, t) + shift
})
const booked = computed(() => {
  const run = glance.run.value
  return run ? formatDateTime(run.processed_at) : ''
})
const next = computed(() => {
  if (automation.value && !automation.value.active) return t('glance.auto.off')
  const item = glance.next.value
  return item ? formatDayMonthShort(item.date) : '—'
})

/** «Автоматизация →» opens its card over this screen; a workflow's record leads to the workflow. */
const toAutomation = computed(() =>
  entityLocation({ kind: automation.value ? 'automation' : 'workflow', id: props.automationId }, route.query),
)
const journal = computed(() => ({ name: 'automations-journal', query: props.runId ? { run: props.runId } : {} }))
function toJournal() {
  open.value = false
  void router.push(journal.value)
}
const amount = computed(() => {
  const leg = txPrincipalLegs(props.tx)[0]
  return leg ? formatMoney(leg.amount, { currency: leg.asset.symbol, signed: Number(leg.amount) > 0 }) : ''
})
const income = computed(() => Number(txPrincipalLegs(props.tx)[0]?.amount ?? 0) > 0)
function correct() {
  open.value = false
  amountOpen.value = true
}
</script>

<template>
  <Glance
    v-model:open="open"
    :title="automation?.name ?? t('glance.auto.title')"
    :open-to="toAutomation"
    :open-label="automation ? t('glance.auto.automation') : t('glance.auto.scheme')"
    trigger-class="rounded-md border border-primary/40 px-1.5 text-xs leading-5"
    kind="auto"
  >
    {{ t('glance.auto.badge') }}
    <template #body>
      <div class="flex items-center gap-2">
        <Badge variant="outline" class="border-primary/40 text-primary">{{ t('glance.auto.badge') }}</Badge>
        <b class="min-w-0 flex-1 truncate">{{ automation?.name ?? t('glance.auto.title') }}</b>
        <b class="shrink-0 text-lg tabular-nums" :class="income && 'text-success'">{{ amount }}</b>
      </div>
      <p v-if="automation" data-glance-auto-who>
        {{ t('glance.auto.who') }} <b>«{{ automation.name }}»</b>{{ booked ? `, ${booked}` : '' }}.
      </p>
      <p v-else-if="!glance.loading.value">{{ t('glance.auto.workflow') }}</p>
      <p v-if="glance.run.value?.estimated" class="text-warning">{{ t('automation.run.outcome.estimated') }}</p>
      <dl v-if="automation" class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5">
        <dt class="text-muted-foreground">{{ t('glance.auto.rule') }}</dt>
        <dd class="text-right">{{ rule }}</dd>
        <dt class="text-muted-foreground">{{ t('glance.auto.amount') }}</dt>
        <dd class="text-right">{{ t(`glance.auto.amount.${automation.amount_mode}`) }}</dd>
        <dt class="text-muted-foreground">{{ t('glance.auto.next') }}</dt>
        <dd class="text-right tabular-nums">{{ next }}</dd>
      </dl>
    </template>
    <template #actions>
      <Button v-if="automation" variant="outline" size="sm" data-glance-auto-amount @click="correct">{{ t('glance.auto.correct') }}</Button>
      <Button variant="link" size="sm" class="h-auto px-0 text-sm" data-glance-auto-journal @click="toJournal">
        {{ t('glance.auto.journal') }} →
      </Button>
    </template>
  </Glance>
  <AutomationAmountDialog
    v-if="automation"
    v-model:open="amountOpen"
    :tx-id="txId(props.tx)"
    :name="automation.name"
    :estimate="glance.run.value?.amount ?? ''"
    :symbol="glance.run.value?.symbol ?? ''"
  />
</template>
