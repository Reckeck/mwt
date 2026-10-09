<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { formatMoney } from '@/shared/lib/format'
import { call } from '@/shared/api/ipc'

const { t } = useI18n()

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})
</script>

<template>
  <Card>
    <template #header>
      <p class="text-sm text-muted-foreground">{{ t('store.total') }}</p>
    </template>
    <div>
      <p class="text-2xl font-bold">
        {{ formatMoney(summary.data.value?.total_base, { currency: summary.data.value?.base_currency }) }}
      </p>
      <p class="text-xs text-muted-foreground">≈ {{ formatMoney(summary.data.value?.total_usd, { currency: 'usd' }) }}</p>
    </div>
  </Card>
</template>
