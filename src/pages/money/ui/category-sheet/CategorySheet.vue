<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { TxRowMenuHost, useTxRowMenu } from '@/features/transaction-row-menu'
import { Money, MoneyPhrase } from '@/entities/currency'
import { TransactionsList, useTxPages } from '@/entities/transaction'
import { call } from '@/shared/api/ipc'
import { drillTo } from '@/shared/lib/drill'
import { pluralForm } from '@/shared/lib/format'
import type { SumPart } from '@/shared/lib/money-parts'
import { rangeDays } from '@/shared/lib/period'
import { openTxDetails } from '@/shared/lib/tx-details'
import { emptyTxFilter } from '@/shared/lib/tx-filter'
import { Button, buttonVariants } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from '@/shared/ui/sheet'

import type { SpendingCategory } from '../../model/use-spending'
import CategoryMonths from './CategoryMonths.vue'

/**
 * A category opened from «Траты» (Money 1 · 2): its sum, the twelve months, its
 * operations, and the two ways on — the journal or a limit — without leaving.
 */
const props = defineProps<{
  row: SpendingCategory | null
  label: string
  /** The period, as the header names it: «октябрь 2026». */
  period: string
  filter: ReportFilter
  months: string[]
  currency: string
  limit: SumPart[]
}>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ limit: [] }>()

const { t, locale } = useI18n()
const days = computed(() => rangeDays({ from: props.filter.from ?? '', to: props.filter.to ?? '' }))
const txFilter = ref<TxFilter>(emptyTxFilter())
watch(
  () => [props.row?.key, days.value.first, days.value.last] as const,
  ([key, from, to]) => {
    txFilter.value = emptyTxFilter({ from, to, category: key ?? null, scope: props.filter.scope })
  },
  { immediate: true },
)
const summary = useQuery({ queryKey: ['summary'], queryFn: () => call<PortfolioSummary>('get_summary') })
const { query: txs, rows, total } = useTxPages(txFilter)
const rowMenu = useTxRowMenu({ duplicate: true })

const operations = (count: number) => t(`money.category.operations.${pluralForm(count, locale.value)}`, { count })
const journal = computed(() =>
  drillTo({ from: props.filter.from ?? undefined, to: props.filter.to ?? undefined, category: props.row?.key ?? '', storeIds: props.filter.scope.store_ids }),
)
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent overlay="category-sheet" side="right" class="flex w-full flex-col gap-0 p-0 sm:max-w-[460px]" data-tour="category-sheet">
      <SheetHeader class="px-6 pt-5 pb-3 max-md:px-4">
        <SheetTitle class="truncate pr-8 text-lg">{{ props.label }}</SheetTitle>
        <SheetDescription>{{ props.period }}</SheetDescription>
      </SheetHeader>
      <div v-if="props.row" class="grid min-h-0 flex-1 content-start gap-3 overflow-y-auto px-6 pb-6 max-md:px-4">
        <section class="grid gap-1 rounded-xl border bg-card p-4 shadow-sm">
          <b class="text-xl tabular-nums"><Money :amount="props.row.total" :currency="props.currency" :decimals="0" /></b>
          <p class="text-sm text-muted-foreground">
            {{ operations(total ?? rows.length) }}
            <template v-if="props.row.average">
              ·
              <i18n-t keypath="money.category.usual">
                <template #average><Money :amount="props.row.average" :currency="props.currency" :decimals="0" /></template>
              </i18n-t>
            </template>
            <template v-if="props.limit.length"> · <MoneyPhrase :parts="props.limit" /></template>
          </p>
        </section>
        <CategoryMonths
          :title="t('money.category.byMonth', { category: props.label })"
          :cells="props.row.cells"
          :months="props.months"
          :average="props.row.average"
          :currency="props.currency"
          :to="props.filter.to ?? ''"
        />
        <SectionState :loading="txs.isPending.value" :error="txs.error.value" skeleton="list" :rows="3" compact @retry="txs.refetch()">
          <TransactionsList
            :transactions="rows"
            :total="total"
            :loading-more="txs.isFetchingNextPage.value"
            openable
            :fx-per-usd="summary.data.value?.fx_per_usd"
            :base-currency="summary.data.value?.base_currency"
            :row-actions="rowMenu.actions"
            @open="openTxDetails"
            @more="txs.fetchNextPage()"
          />
        </SectionState>
      </div>
      <SheetFooter class="flex-row justify-between gap-2 border-t px-6 py-3 max-md:px-4">
        <RouterLink :to="journal" :class="buttonVariants({ variant: 'outline', class: 'max-md:h-11' })" @click="open = false">
          {{ t('money.category.toJournal') }}
        </RouterLink>
        <Button class="max-md:h-11 max-md:flex-1" data-category-sheet-limit @click="emit('limit')">
          {{ props.row?.budget ? t('money.categories.editLimit') : t('money.categories.setLimit') }}
        </Button>
      </SheetFooter>
    </SheetContent>
    <TxRowMenuHost :menu="rowMenu" />
  </Sheet>
</template>
