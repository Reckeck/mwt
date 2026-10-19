<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ChevronDown } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDate, formatDateTime, formatMoney } from '@/shared/lib/format'
import { txId, txKind, txMeta, txPrincipalLegs } from '@/shared/lib/transactions'
import { Button } from '@/shared/ui/button'
import { openTxDetails } from '@/shared/lib/tx-details'

/** Months (`YYYY-MM`, inclusive) and wallets the chains must fall in; none = all of them. */
const props = withDefaults(defineProps<{ from?: string | null; to?: string | null; storeIds?: string[] }>(), {
  from: null,
  to: null,
  storeIds: () => [],
})

const { t } = useI18n()

/** The start of a month, local time, as the backend's RFC3339 bound. */
const monthStart = (key: string, shift = 0) => {
  const [year, month] = key.split('-').map(Number)
  return new Date(year, month - 1 + shift, 1).toISOString()
}
const range = computed(() => ({
  from: props.from ? monthStart(props.from) : null,
  to: props.to ? monthStart(props.to, 1) : null,
  storeIds: props.storeIds.length ? props.storeIds : null,
}))
const flows = useQuery({
  queryKey: computed(() => ['flows', range.value]),
  queryFn: () => call<FlowSummary[]>('list_flows', range.value),
})
const expanded = ref<string | null>(null)

const amounts = (list: FlowAmount[]) => list.map((item) => `${item.amount} ${item.asset?.symbol ?? ''}`).join(' + ') || '—'
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('flows.listTitle') }}</p>
      <p class="text-xs text-muted-foreground">{{ t('flows.listHint') }}</p>
    </template>
    <p v-if="flows.isSuccess.value && !flows.data.value?.length" class="text-sm text-muted-foreground">{{ t('flows.empty') }}</p>
    <ul class="grid gap-2">
      <li v-for="flow in flows.data.value ?? []" :key="flow.flow_id" class="rounded-md border border-border">
        <Button
          variant="ghost"
          size="block"
          class="rounded-none flex w-full flex-wrap items-center justify-between gap-2 px-3 py-2 text-left text-sm"
          :aria-expanded="expanded === flow.flow_id"
          @click="expanded = expanded === flow.flow_id ? null : flow.flow_id"
        >
          <span class="flex min-w-0 items-center gap-2">
            <ChevronDown class="size-4 shrink-0 transition-transform" :class="expanded === flow.flow_id && 'rotate-180'" />
            <span class="truncate tabular-nums">{{ amounts(flow.gave) }} → {{ amounts(flow.got) }}</span>
          </span>
          <span class="shrink-0 text-xs tabular-nums text-muted-foreground">
            {{ formatDate(flow.started_at) }} · {{ t('flows.links', { count: flow.links.length }) }} ·
            <span class="text-warning"><Money :amount="flow.loss" :currency="flow.currency" /></span>
          </span>
        </Button>
        <div v-if="expanded === flow.flow_id" class="grid gap-2 border-t border-border px-3 py-2 text-xs">
          <ol class="grid gap-1">
            <li v-for="(link, index) in flow.links" :key="txId(link.tx)" class="flex justify-between gap-2">
              <Button
                variant="link"
                class="h-auto min-w-0 justify-start truncate p-0 text-xs font-normal text-foreground"
                :title="t('tx.openDetails')"
                @click="openTxDetails(link.tx)"
              >
                {{ index + 1 }}. {{ t(`tx.kind.${txKind(link.tx)}`) }} · {{ formatDateTime(txMeta(link.tx).timestamp) }} ·
                {{ txPrincipalLegs(link.tx).map((leg) => `${leg.amount} ${leg.asset.symbol}`).join(' → ') }}
              </Button>
              <span class="shrink-0 tabular-nums"><Money :amount="link.loss.total" :currency="flow.currency" /></span>
            </li>
          </ol>
          <p class="tabular-nums text-muted-foreground">
            {{ t('flows.result', { gave: formatMoney(flow.gave_value, { currency: flow.currency }), got: formatMoney(flow.got_value, { currency: flow.currency }), loss: formatMoney(flow.loss, { currency: flow.currency }) }) }}
          </p>
          <p v-if="flow.difference !== flow.loss" class="text-muted-foreground">
            {{ t('flows.difference', { amount: formatMoney(flow.difference, { currency: flow.currency }) }) }}
          </p>
        </div>
      </li>
    </ul>
  </Card>
</template>
