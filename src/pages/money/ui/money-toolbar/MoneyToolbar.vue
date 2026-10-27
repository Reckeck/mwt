<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { SlidersHorizontal } from 'lucide-vue-next'

import { ChartCurrencySwitch, CurrencyPicker } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { ReportExportButton } from '@/features/report-export'
import { useStoreTree } from '@/shared/api/queries'
import { useIsPhone } from '@/shared/lib/phone'
import type { MonthRange } from '@/shared/lib/period'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'
import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'
import { PeriodPicker } from '@/shared/ui/period-picker'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { Switch } from '@/shared/ui/switch'

/**
 * The tab row's right side (the canvas toolbar): ‹ period ›, the wallets, the
 * rare filters behind one button, the export. One filter for the whole section.
 * On a phone it is one row: the period, one filters button that holds the wallets too,
 * and «⋯» with the export.
 * Where the tab asks for it, the budget currency or the base one sits before the
 * period (Currency-1 · 7: «₽ | $»).
 */
const props = defineProps<{
  fallback: MonthRange
  realTerms: boolean
  wallets: boolean
  exportKinds: ReportKind[]
  effective: ReportFilter
  currencySwitch?: boolean
}>()
const filter = defineModel<ReportFilter>({ required: true })
const range = defineModel<MonthRange>('range', { required: true })

const { t } = useI18n()
const phone = useIsPhone()
const route = useRoute()
const tree = useStoreTree()
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))
const storeIds = computed({
  get: () => filter.value.scope.store_ids,
  set: (ids: string[]) => {
    filter.value.scope = { store_ids: ids, include_children: true }
  },
})
const settings = useSettingsStore()
const budgetCurrency = computed(() => (settings.vault?.budget_currency || settings.baseCurrency).toLowerCase())
const currencies = computed(() => [...new Set([budgetCurrency.value, settings.baseCurrency.toLowerCase()])])
/** The budget currency is the report's own: picking it clears the override. */
const shownCurrency = computed({
  get: () => filter.value.currency?.toLowerCase() || budgetCurrency.value,
  set: (code: string) => {
    if (code) filter.value.currency = code === budgetCurrency.value ? null : code
  },
})
/** Set filters behind the button: a dot shows that something narrows the tab. */
const extras = computed(
  () => (filter.value.currency ? 1 : 0) + (props.realTerms && filter.value.real_terms ? 1 : 0) + (phone.value && storeIds.value.length ? 1 : 0),
)
</script>

<template>
  <div :class="phone ? 'flex w-full min-w-0 items-center gap-2' : 'flex min-w-0 flex-wrap items-center gap-2'" data-tour="report-filter">
    <div v-if="props.currencySwitch" class="max-md:hidden" data-tour="spending-currency">
      <ChartCurrencySwitch v-model="shownCurrency" :options="currencies" />
    </div>
    <div data-tour="period-picker" class="min-w-0 max-md:flex-1">
      <PeriodPicker v-model="range" :default-range="props.fallback" trigger-class="w-[170px] max-md:flex-1" />
    </div>
    <StoreNodeSelect
      v-if="props.wallets && !phone"
      id="money-scope"
      v-model:ids="storeIds"
      multiple
      :nodes="nodes"
      :empty-label="t('report.filter.allWallets')"
      :aria-label="t('report.filter.scope')"
      class="w-[180px] max-md:flex-1 pointer-coarse:h-10"
    />
    <Popover v-if="props.wallets">
      <PopoverTrigger as-child>
        <Button variant="outline" size="icon" class="relative" :aria-label="t('money.moreFilters')" :title="t('money.moreFilters')">
          <SlidersHorizontal />
          <span v-if="extras" class="absolute top-1.5 right-1.5 size-1.5 rounded-full bg-primary" />
        </Button>
      </PopoverTrigger>
      <PopoverContent overlay="money-wallets-filter" align="end" class="grid w-[min(18rem,100vw-2rem)] gap-4">
        <div v-if="phone" class="grid gap-1.5">
          <Label>{{ t('report.filter.scope') }}</Label>
          <div class="grid max-h-44 overflow-y-auto">
            <StoreNodeSelect v-model:ids="storeIds" multiple inline :nodes="nodes" :empty-label="t('report.filter.allWallets')" />
          </div>
        </div>
        <div class="grid gap-1.5">
          <Label for="money-currency">{{ t('report.filter.currency') }}</Label>
          <CurrencyPicker
            id="money-currency"
            allow-empty
            :placeholder="t('report.filter.budgetCurrency')"
            :model-value="filter.currency ?? ''"
            @update:model-value="(value) => (filter.currency = String(value) || null)"
          />
        </div>
        <div v-if="props.realTerms" class="grid gap-1.5" data-tour="real-terms">
          <div class="flex items-center gap-2">
            <Switch id="money-real-terms" v-model="filter.real_terms" />
            <Label for="money-real-terms">{{ t('analytics.realTerms') }}</Label>
          </div>
          <p class="text-xs text-muted-foreground">{{ t('analytics.realTermsHint') }}</p>
          <RouterLink
            v-if="filter.real_terms"
            :to="{ query: { ...route.query, settings: 'accounting', section: 'inflation' } }"
            class="text-xs text-muted-foreground underline"
          >
            {{ t('analytics.realTermsRate') }}
          </RouterLink>
        </div>
      </PopoverContent>
    </Popover>
    <div v-if="props.exportKinds.length" data-tour="report-export">
      <ReportExportButton :kinds="props.exportKinds" :filter="props.effective" />
    </div>
  </div>
</template>
