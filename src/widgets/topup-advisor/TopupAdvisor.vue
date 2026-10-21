<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Settings2 } from 'lucide-vue-next'

import { AssetClassDialog } from '@/features/asset-class-edit'
import { assetClassName } from '@/entities/asset-class'
import { useAssetClasses } from '@/shared/api/queries'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatMonthKey } from '@/shared/lib/format'
import { useTheme } from '@/shared/lib/theme'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { FormField } from '@/shared/ui/form-field'
import { SectionState } from '@/shared/ui/section-state'

import { advisorChart } from './model/advisor-chart'
import { useTopupAdvice } from './model/use-topup-advice'
import AdvisorRows from './ui/AdvisorRows.vue'
import AssetBaskets from './ui/AssetBaskets.vue'

/** Where this month's free money should go so the baskets come closer to their shares. Creates nothing. */
const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)
const view = useAssetClasses()
const { amountText, error, advice } = useTopupAdvice()
const editOpen = ref(false)

const amountHint = computed(() => {
  const data = advice.data.value
  if (!data?.from_savings) return t('assetClass.advisor.typedHint')
  return data.savings_month && Number(data.amount) > 0
    ? t('assetClass.advisor.fromSavings', { month: formatMonthKey(data.savings_month, { month: 'long', year: 'numeric' }) })
    : t('assetClass.advisor.noSavings')
})

const option = computed<ChartOption>(() =>
  advisorChart(
    (advice.data.value?.rows ?? []).map((row) => ({
      name: assetClassName(row.class, t),
      target: Number(row.class.target_pct),
      now: row.pct_before === null ? null : Number(row.pct_before),
      after: row.pct_after === null ? null : Number(row.pct_after),
    })),
    { target: t('assetClass.chart.target'), now: t('assetClass.chart.now'), after: t('assetClass.chart.after') },
    { axisColor: axisColor.value },
  ),
)
</script>

<template>
  <div class="grid grid-cols-1 gap-4 lg:grid-cols-2">
    <Card data-tour="topup-advisor">
      <template #header>
        <div class="flex items-center justify-between gap-2">
          <p class="font-medium">{{ t('assetClass.advisor.title') }}</p>
          <Button variant="outline" size="sm" data-tour="asset-classes" @click="editOpen = true">
            <Settings2 class="size-4" />
            {{ t('assetClass.advisor.edit') }}
          </Button>
        </div>
      </template>
      <div class="grid gap-4">
        <FormField for="topup-amount" :label="t('assetClass.advisor.amount')" :hint="amountHint" :error="error ?? undefined">
          <AmountInput
            id="topup-amount"
            v-model="amountText"
            :decimals="2"
            :placeholder="advice.data.value?.amount ?? ''"
          />
        </FormField>
        <SectionState :loading="advice.isPending.value" :error="advice.error.value" skeleton="report" @retry="advice.refetch()">
          <div v-if="advice.data.value" class="grid gap-3">
            <AdvisorRows :advice="advice.data.value" />
            <p v-if="advice.data.value.preset" class="text-xs text-muted-foreground">{{ t('assetClass.advisor.presetNote') }}</p>
          </div>
        </SectionState>
      </div>
    </Card>
    <Card>
      <template #header>
        <p class="font-medium">{{ t('assetClass.chart.title') }}</p>
      </template>
      <div class="h-56 w-full"><Chart :option="option" autoresize /></div>
    </Card>
    <Card class="lg:col-span-2" data-tour="asset-baskets">
      <template #header>
        <p class="font-medium">{{ t('assetClass.assets.title') }}</p>
      </template>
      <SectionState :loading="view.isPending.value" :error="view.error.value" skeleton="list" @retry="view.refetch()">
        <AssetBaskets v-if="view.data.value" :view="view.data.value" />
      </SectionState>
    </Card>
    <AssetClassDialog v-model:open="editOpen" :view="view.data.value" />
  </div>
</template>
