<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'
import { RefreshCw } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { formatDate } from '@/shared/lib/format'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'
import { AdvancedFields } from '@/shared/ui/advanced-fields'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import type { InvestCalc } from '../model/use-invest-calc'

/**
 * «Из данных»: the start amount and rate from chosen wallets, the
 * contribution as a share of the average income. A snapshot — the numbers
 * change only on «Обновить из данных» (decision 10).
 */
const props = defineProps<{ calc: InvestCalc }>()

const { t } = useI18n()
const settings = useSettingsStore()
const tree = useStoreTree()
const liveNodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const INCOME_MONTHS = [3, 6, 12] as const
const defaultPct = () => String(Math.round(Number(settings.vault?.savings_rate ?? '0.1') * 100))

const storeIds = ref<string[]>([])
const months = ref<number>(12)
const pct = ref(defaultPct())
const last = ref<ScenarioPrefill | null>(null)

// A loaded scenario brings its own source.
watch(
  () => props.calc.source.value,
  (source) => {
    storeIds.value = source ? [...source.scope.store_ids] : []
    months.value = source?.income_months ?? 12
    pct.value = source?.income_pct ?? defaultPct()
    if (!source) last.value = null
  },
  { immediate: true },
)

const prefill = useMutation({
  mutationFn: (source: ScenarioSource) =>
    call<ScenarioPrefill>('calc_scenario_prefill', { source, currency: props.calc.form.currency }),
  onSuccess: (data, source) => {
    last.value = data
    props.calc.applyPrefill(source, data)
  },
  meta: { error: 'invest.prefillFailed', success: 'invest.prefilled' },
})

function run() {
  prefill.mutate({
    scope: { store_ids: [...storeIds.value], include_children: false },
    income_months: months.value,
    income_pct: pct.value.trim().replace(',', '.') || '0',
  })
}

function pickMonths(value: unknown) {
  if (typeof value === 'string' && value) months.value = Number(value)
}
</script>

<template>
  <AdvancedFields :label="t('invest.prefill')" :default-open="props.calc.source.value !== null">
    <div class="grid gap-3" data-tour="calc-prefill">
      <p class="text-sm text-muted-foreground">{{ t('invest.prefillHint') }}</p>
      <FormField for="calc-prefill-stores" :label="t('invest.prefillStores')" :hint="t('invest.prefillStoresHint')">
        <StoreNodeSelect
          id="calc-prefill-stores"
          v-model:ids="storeIds"
          multiple
          flat
          :nodes="liveNodes"
          :empty-label="t('invest.prefillNoStores')"
        />
      </FormField>
      <div class="grid gap-3 @md:grid-cols-2">
        <FormField :label="t('invest.prefillMonths')">
          <ToggleGroup type="single" variant="outline" size="sm" :model-value="String(months)" @update:model-value="pickMonths">
            <ToggleGroupItem v-for="count in INCOME_MONTHS" :key="count" :value="String(count)">
              {{ t('invest.monthsShort', { count }) }}
            </ToggleGroupItem>
          </ToggleGroup>
        </FormField>
        <FormField for="calc-prefill-pct" :label="t('invest.prefillPct')" :hint="t('invest.prefillPctHint')">
          <AmountInput id="calc-prefill-pct" v-model="pct" :decimals="2" placeholder="10" />
        </FormField>
      </div>
      <div class="flex flex-wrap items-center gap-3">
        <Button variant="outline" size="sm" :disabled="prefill.isPending.value" @click="run">
          <RefreshCw class="size-4" :class="prefill.isPending.value && 'animate-spin'" />{{ t('invest.prefillRun') }}
        </Button>
        <p v-if="props.calc.prefilledAt.value" class="text-xs text-muted-foreground">
          {{ t('invest.prefilledAt', { date: formatDate(props.calc.prefilledAt.value) }) }}
        </p>
      </div>
      <p v-if="last?.income_avg" class="text-xs text-muted-foreground">
        {{ last.income_manual ? t('invest.prefillIncomeManual') : t('invest.prefillIncome', { months: last.income_months_used }) }}
        <Money :amount="last.income_avg" :currency="props.calc.form.currency" />
      </p>
      <p v-else-if="last" class="text-xs text-muted-foreground">{{ t('invest.prefillNoIncome') }}</p>
      <p v-if="last?.pays_out" class="text-xs text-warning">{{ t('invest.prefillPaysOut') }}</p>
    </div>
  </AdvancedFields>
</template>
