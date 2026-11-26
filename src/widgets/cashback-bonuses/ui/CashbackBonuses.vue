<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, useRoute } from 'vue-router'
import { CreditCard, Sparkles } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { useCashbackReport } from '@/shared/api/queries'
import { entityLocation } from '@/shared/lib/entity-link'
import { formatDate, formatQuantity } from '@/shared/lib/format'
import { Card } from '@/shared/ui/card'
import { ListRow } from '@/shared/ui/list-row'
import { SectionState } from '@/shared/ui/section-state'
import { Skeleton } from '@/shared/ui/skeleton'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

/** Cashback and points of the month or the year, by card, and the programmes' balances. */
const { t } = useI18n()
const route = useRoute()
const period = ref<'month' | 'year'>('month')
const report = useCashbackReport(period)
const data = computed(() => report.data.value ?? null)
const total = computed(() => (data.value ? Number(data.value.cashback_base) + Number(data.value.points_base) : 0))
const soon = (iso: string | null) => !!iso && new Date(iso).getTime() - Date.now() < 30 * 86_400_000
const storeLink = (id: string) => entityLocation({ kind: 'store', id }, route.query)
</script>

<template>
  <Card size="sm">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p data-widget-title class="text-sm text-muted-foreground">{{ t('cashback.title') }}</p>
        <ToggleGroup v-model="period" type="single" size="sm" variant="outline" :aria-label="t('cashback.title')">
          <ToggleGroupItem value="month">{{ t('cashback.month') }}</ToggleGroupItem>
          <ToggleGroupItem value="year">{{ t('cashback.year') }}</ToggleGroupItem>
        </ToggleGroup>
      </div>
    </template>
    <Skeleton v-if="report.isPending.value" class="h-20 w-full" />
    <SectionState v-else-if="report.error.value" :error="report.error.value" compact @retry="report.refetch()" />
    <div v-else-if="data" class="grid gap-3 text-sm">
      <div class="flex flex-wrap items-baseline justify-between gap-2">
        <Money :amount="total" :currency="data.base_currency" class="text-xl font-semibold @xs:text-2xl" />
        <span class="text-xs text-muted-foreground">
          {{ t('cashback.cashback') }} <Money :amount="data.cashback_base" :currency="data.base_currency" :convert="false" />
          · {{ t('cashback.points') }} <Money :amount="data.points_base" :currency="data.base_currency" :convert="false" />
        </span>
      </div>
      <p v-if="!data.by_source.length && !data.programmes.length" class="text-xs text-muted-foreground">{{ t('cashback.empty') }}</p>

      <div v-if="data.by_source.length" class="grid">
        <p class="text-xs text-muted-foreground">{{ t('cashback.bySource') }}</p>
        <ul class="divide-y divide-border">
          <li v-for="row in data.by_source" :key="row.store_id ?? 'none'">
            <component :is="row.store_id ? RouterLink : 'div'" :to="row.store_id ? storeLink(row.store_id) : undefined" class="block">
              <ListRow :title="row.name || t('cashback.unknownSource')" :icon="CreditCard">
                <template #value>
                  <Money :amount="Number(row.cashback_base) + Number(row.points_base)" :currency="data.base_currency" :convert="false" />
                </template>
              </ListRow>
            </component>
          </li>
        </ul>
      </div>

      <div v-if="data.programmes.length" class="grid">
        <p class="text-xs text-muted-foreground">{{ t('cashback.programmes') }}</p>
        <ul class="divide-y divide-border">
          <li v-for="row in data.programmes" :key="row.store_id">
            <RouterLink :to="storeLink(row.store_id)" class="block">
              <ListRow
                :title="row.name"
                :icon="Sparkles"
                :subtitle="row.expires_at ? t('loyalty.expiring', { points: row.expiring_points ?? '', date: formatDate(row.expires_at) }) : undefined"
                :class="soon(row.expires_at) && '[&_[data-list-row-subtitle]]:text-warning'"
              >
                <template #value>
                  {{ formatQuantity(row.points, 0) }} {{ row.points_symbol }}
                  <span class="block text-xs font-normal text-muted-foreground">
                    ≈ <Money :amount="row.value_base" :currency="data.base_currency" :convert="false" />
                  </span>
                </template>
              </ListRow>
            </RouterLink>
          </li>
        </ul>
      </div>
    </div>
  </Card>
</template>
