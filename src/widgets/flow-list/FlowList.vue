<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ChevronDown } from 'lucide-vue-next'

import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDate, formatDateTime, formatMoney } from '@/shared/lib/format'
import { txId, txKind, txMeta, txPrincipalLegs } from '@/shared/lib/transactions'

const { t } = useI18n()

const flows = useQuery({ queryKey: ['flows'], queryFn: () => call<FlowSummary[]>('list_flows', { from: null, to: null }) })
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
        <button
          type="button"
          class="flex w-full flex-wrap items-center justify-between gap-2 px-3 py-2 text-left text-sm"
          :aria-expanded="expanded === flow.flow_id"
          @click="expanded = expanded === flow.flow_id ? null : flow.flow_id"
        >
          <span class="flex min-w-0 items-center gap-2">
            <ChevronDown class="size-4 shrink-0 transition-transform" :class="expanded === flow.flow_id && 'rotate-180'" />
            <span class="truncate tabular-nums">{{ amounts(flow.gave) }} → {{ amounts(flow.got) }}</span>
          </span>
          <span class="shrink-0 text-xs tabular-nums text-muted-foreground">
            {{ formatDate(flow.started_at) }} · {{ t('flows.links', { count: flow.links.length }) }} ·
            <span class="text-amber-600 dark:text-amber-400">{{ formatMoney(flow.loss, { currency: flow.currency }) }}</span>
          </span>
        </button>
        <div v-if="expanded === flow.flow_id" class="grid gap-2 border-t border-border px-3 py-2 text-xs">
          <ol class="grid gap-1">
            <li v-for="(link, index) in flow.links" :key="txId(link.tx)" class="flex justify-between gap-2">
              <span class="min-w-0 truncate">
                {{ index + 1 }}. {{ t(`tx.kind.${txKind(link.tx)}`) }} · {{ formatDateTime(txMeta(link.tx).timestamp) }} ·
                {{ txPrincipalLegs(link.tx).map((leg) => `${leg.amount} ${leg.asset.symbol}`).join(' → ') }}
              </span>
              <span class="shrink-0 tabular-nums">{{ formatMoney(link.loss.total, { currency: flow.currency }) }}</span>
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
