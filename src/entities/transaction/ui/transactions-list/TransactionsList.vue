<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { txId } from '@/shared/lib/transactions'
import { provideTxContext } from '@/shared/lib/tx-context'

import { dayLabel } from '../../lib/day-label'
import TransactionRow from '../transaction-row/TransactionRow.vue'

import { groupByDay } from './group-by-day'
import type { TransactionsListProps } from './transactions-list.types'
import { TX_PAGE } from './use-tx-pages'

const props = withDefaults(defineProps<TransactionsListProps>(), { losses: null, fxPerUsd: null, baseCurrency: undefined, loadingMore: false })
const emit = defineEmits<{ open: [tx: Transaction, list: Transaction[]]; more: [] }>()

const { t } = useI18n()
const context = provideTxContext()

/** The day totals are in the base currency: they wait for its code and its rate. */
const ratesReady = computed(() => Boolean(props.baseCurrency) && props.fxPerUsd !== null && props.fxPerUsd !== undefined)
const days = computed(() => groupByDay(props.transactions, Number(props.fxPerUsd ?? 1), context.rootOf))
const rest = computed(() => props.total - props.transactions.length)
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)]">
    <section v-for="day in days" :key="day.date" data-tx-day>
      <!-- Opaque, no backdrop-blur: a blur on every day header froze WebKitGTK for good. The header is a row of its own: rows never slide under it unseen. -->
      <h3
        class="sticky top-0 z-[1] flex min-h-9 items-center justify-between gap-2 border-b border-border bg-background px-1 text-xs font-medium text-muted-foreground"
        data-tx-day-header
      >
        <span>{{ dayLabel(`${day.date}T12:00:00`) }}</span>
        <!-- No total until the summary brought the base code and rate: a USD sum under «₽» would be a lie. -->
        <span v-if="ratesReady" class="tabular-nums" :title="day.partial ? t('tx.dayTotalPartial') : t('tx.dayTotal')">
          <Money :amount="day.totalBase" :currency="props.baseCurrency" :decimals="2" signed />
          <template v-if="day.partial">*</template>
        </span>
      </h3>
      <div class="divide-y divide-border">
        <TransactionRow
          v-for="tx in day.txs"
          :key="txId(tx)"
          :data-tx-row="txId(tx)"
          class="min-w-0 px-1"
          :tx="tx"
          :loss="props.losses?.[txId(tx)] ?? null"
          :dated="false"
          openable
          @open="emit('open', $event, props.transactions)"
        />
      </div>
    </section>

    <div v-if="rest > 0" class="flex justify-center py-3">
      <Button variant="outline" size="sm" class="touch-min" :disabled="props.loadingMore" @click="emit('more')">
        {{ t('ui.showMore', { count: Math.min(rest, TX_PAGE) }) }}
      </Button>
    </div>
  </div>
</template>
