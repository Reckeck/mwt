<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { formatDay } from '@/shared/lib/format'

import { groupByDay } from '../lib/group-by-day'
import { CRYPTO_OPS_PAGE } from '../model/use-crypto-ops'
import CryptoOpRow from './CryptoOpRow.vue'

/** The operations grouped by day, as the operations list is; `Show more` brings the next page. */
const props = defineProps<{ rows: CryptoOpRow[]; total: number; loadingMore?: boolean }>()
const emit = defineEmits<{ more: [] }>()
const { t } = useI18n()

const days = computed(() => groupByDay(props.rows))
const rest = computed(() => props.total - props.rows.length)
const dayLabel = (date: string) => formatDay(`${date}T12:00:00`).replaceAll('.', '')
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)]">
    <section v-for="day in days" :key="day.date" data-crypto-op-day>
      <h3 class="sticky top-0 z-[1] flex min-h-9 items-center justify-between gap-2 border-b border-border bg-background px-1 text-xs font-medium text-muted-foreground">
        <span class="capitalize">{{ dayLabel(day.date) }}</span>
        <span v-if="day.total !== null" class="tabular-nums"><Money :amount="day.total" currency="usd" :decimals="2" signed /></span>
      </h3>
      <div class="divide-y divide-border">
        <CryptoOpRow v-for="row in day.rows" :key="row.id" :row="row" />
      </div>
    </section>
    <div v-if="rest > 0" class="flex justify-center py-3">
      <Button variant="outline" size="sm" class="touch-min" :disabled="props.loadingMore" data-crypto-op-more @click="emit('more')">
        {{ t('ui.showMore', { count: Math.min(rest, CRYPTO_OPS_PAGE) }) }}
      </Button>
    </div>
  </div>
</template>
