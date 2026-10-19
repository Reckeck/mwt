<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { call } from '@/shared/api/ipc'
import { formatDate, formatQuantity } from '@/shared/lib/format'

/** Cashback and points of the month or the year, by card, and the programmes' balances. */
const { t } = useI18n()
const period = ref<'month' | 'year'>('month')
const report = useQuery({
  queryKey: computed(() => ['cashback', period.value]),
  queryFn: () => call<CashbackReport>('get_cashback_report', { period: period.value }),
})
const data = computed(() => report.data.value ?? null)
const total = computed(() => (data.value ? Number(data.value.cashback_base) + Number(data.value.points_base) : 0))
const soon = (iso: string | null) => !!iso && new Date(iso).getTime() - Date.now() < 30 * 86_400_000
</script>

<template>
  <Card size="sm">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p data-widget-title class="text-sm text-muted-foreground">{{ t('cashback.title') }}</p>
        <ToggleGroup v-model="period" type="single" size="sm" variant="outline" :aria-label="t('cashback.title')">
          <ToggleGroupItem value="month" class="h-6 px-2 text-xs">{{ t('cashback.month') }}</ToggleGroupItem>
          <ToggleGroupItem value="year" class="h-6 px-2 text-xs">{{ t('cashback.year') }}</ToggleGroupItem>
        </ToggleGroup>
      </div>
    </template>
    <Skeleton v-if="report.isPending.value" class="h-20 w-full" />
    <div v-else-if="data" class="grid gap-3 text-sm">
      <div class="flex items-baseline justify-between gap-2">
        <Money :amount="total" :currency="data.base_currency" class="text-xl font-semibold @xs:text-2xl" />
        <span class="text-xs text-muted-foreground">
          {{ t('cashback.cashback') }} <Money :amount="data.cashback_base" :currency="data.base_currency" :convert="false" />
          · {{ t('cashback.points') }} <Money :amount="data.points_base" :currency="data.base_currency" :convert="false" />
        </span>
      </div>
      <p v-if="!data.by_source.length && !data.programmes.length" class="text-xs text-muted-foreground">{{ t('cashback.empty') }}</p>
      <ul v-if="data.by_source.length" class="grid gap-1">
        <li v-for="row in data.by_source" :key="row.store_id ?? 'none'" class="flex items-center justify-between gap-2">
          <span class="truncate">{{ row.name || t('cashback.unknownSource') }}</span>
          <Money
            :amount="Number(row.cashback_base) + Number(row.points_base)"
            :currency="data.base_currency"
            :convert="false"
            class="text-muted-foreground"
          />
        </li>
      </ul>
      <ul v-if="data.programmes.length" class="grid gap-1 border-t border-border pt-2">
        <li v-for="row in data.programmes" :key="row.store_id" class="grid gap-0.5">
          <div class="flex items-center justify-between gap-2">
            <span class="truncate">{{ row.name }}</span>
            <span class="tabular-nums">
              {{ formatQuantity(row.points, 0) }} {{ row.points_symbol }}
              <span class="text-muted-foreground">≈ <Money :amount="row.value_base" :currency="data.base_currency" :convert="false" /></span>
            </span>
          </div>
          <p v-if="row.expires_at" class="text-xs" :class="soon(row.expires_at) ? 'text-warning' : 'text-muted-foreground'">
            {{ t('loyalty.expiring', { points: row.expiring_points ?? '', date: formatDate(row.expires_at) }) }}
          </p>
        </li>
      </ul>
    </div>
  </Card>
</template>
