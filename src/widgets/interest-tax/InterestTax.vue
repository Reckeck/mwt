<script setup lang="ts">
import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'

/**
 * НДФЛ on deposit interest: how much of this year's tax-free limit
 * (1 000 000 ₽ × the highest key rate) is used, and the tax above it.
 */
const { t } = useI18n()
const queryClient = useQueryClient()

const tax = useQuery({ queryKey: ['interest-tax'], queryFn: () => call<InterestTaxReport>('get_interest_tax', { year: null }) })
const refresh = useMutation({
  mutationFn: () => call<number>('refresh_key_rates'),
  onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['interest-tax'] }),
  meta: { success: 'interestTax.refreshed', error: 'interestTax.keyRateFailed' },
})
// Without the key rate there is no limit: fetch it once, quietly.
watch(
  () => tax.data.value,
  (data) => {
    if (data && data.max_key_rate === null && !refresh.isPending.value && !refresh.isError.value) refresh.mutate()
  },
)

const used = computed(() => {
  const data = tax.data.value
  if (!data?.limit || !Number(data.limit)) return 0
  return Math.min(100, (Number(data.income) / Number(data.limit)) * 100)
})
</script>

<template>
  <Card size="sm">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p data-widget-title class="text-sm text-muted-foreground">{{ t('interestTax.title', { year: tax.data.value?.year ?? '' }) }}</p>
        <span v-if="tax.data.value?.max_key_rate" class="text-xs text-muted-foreground">
          {{ t('deposit.keyRate', { rate: tax.data.value.max_key_rate }) }}
        </span>
      </div>
    </template>
    <Skeleton v-if="tax.isPending.value" class="h-16 w-full" />
    <div v-else-if="tax.data.value" class="grid gap-2 text-sm">
      <div class="flex items-baseline justify-between gap-2">
        <span class="text-muted-foreground">{{ t('interestTax.income') }}</span>
        <Money :amount="tax.data.value.income" currency="rub" class="font-semibold" />
      </div>
      <template v-if="tax.data.value.limit">
        <div class="h-1.5 overflow-hidden rounded-full bg-muted">
          <div
            class="h-full rounded-full"
            :class="used >= 100 ? 'bg-destructive' : used > 80 ? 'bg-warning' : 'bg-primary'"
            :style="{ width: `${used}%` }"
          />
        </div>
        <div class="flex items-baseline justify-between gap-2 text-xs text-muted-foreground">
          <span>{{ t('interestTax.limit') }} <Money :amount="tax.data.value.limit" currency="rub" :decimals="0" /></span>
          <span v-if="tax.data.value.remaining_free && Number(tax.data.value.remaining_free) > 0">
            {{ t('interestTax.remaining') }} <Money :amount="tax.data.value.remaining_free" currency="rub" :decimals="0" />
          </span>
        </div>
        <div v-if="Number(tax.data.value.tax) > 0" class="flex items-baseline justify-between gap-2">
          <span class="text-destructive">{{ t('interestTax.tax') }}</span>
          <Money :amount="tax.data.value.tax" currency="rub" class="font-semibold text-destructive" />
        </div>
      </template>
      <p v-else class="text-xs text-muted-foreground">
        {{ refresh.isError.value ? t('interestTax.keyRateFailed') : t('interestTax.keyRateLoading') }}
      </p>
    </div>
  </Card>
</template>
