<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'

const { t } = useI18n()

// Assets include property and what people owe; liabilities are loans and
// what I owe — so the net figure is what the user is actually worth.
const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})

const currency = computed(() => summary.data.value?.base_currency ?? '')
const hasDebts = computed(() => Number(summary.data.value?.liabilities_base ?? 0) !== 0)
</script>

<template>
  <Card size="sm">
    <template #header>
      <p class="text-sm text-muted-foreground">{{ t('netWorth.title') }}</p>
    </template>
    <div class="grid gap-3">
      <div>
        <p class="text-2xl font-bold">
          <Money :amount="summary.data.value?.net_worth_base ?? '0'" :currency="currency" />
        </p>
      </div>
      <div class="grid grid-cols-2 gap-3 text-sm">
        <div>
          <p class="text-xs text-muted-foreground">{{ t('netWorth.assets') }}</p>
          <p class="font-medium"><Money :amount="summary.data.value?.assets_base ?? '0'" :currency="currency" /></p>
        </div>
        <div>
          <p class="text-xs text-muted-foreground">{{ t('netWorth.liabilities') }}</p>
          <p class="font-medium" :class="hasDebts && 'text-destructive'">
            <Money :amount="summary.data.value?.liabilities_base ?? '0'" :currency="currency" />
          </p>
        </div>
      </div>
    </div>
  </Card>
</template>
