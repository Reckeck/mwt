<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency/@x/crypto-tx'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import { formatDay } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'

import { CRYPTO_OPS_PAGE } from '../../model/use-crypto-ops'
import { opTitle, shortAddress } from '../../lib/op-view'
import CryptoTxRow from '../crypto-tx-row/CryptoTxRow.vue'
import type { CryptoTxListProps } from './crypto-tx-list.types'

/**
 * The operations grouped by day: a strip with the day and its total, then the rows. The same shape as the
 * app's operations list. It renders the pages the backend has sent so far (a page is 100 rows) and offers
 * `Show more` for the rest, so ten thousand operations never mount at once.
 */
const props = withDefaults(defineProps<CryptoTxListProps>(), { loadingMore: false, rowActions: undefined })
const emit = defineEmits<{ open: [row: CryptoOpRow]; more: [] }>()

const { t } = useI18n()
const phone = useIsPhone()

interface Day {
  date: string
  rows: CryptoOpRow[]
  /** USD value of what has a price; `null` when none has. */
  total: number | null
}
/** Rows arrive newest first; a new day starts where the date changes. */
const days = computed<Day[]>(() => {
  const out: Day[] = []
  for (const row of props.rows) {
    const date = row.time.slice(0, 10)
    let day = out.at(-1)
    if (!day || day.date !== date) {
      day = { date, rows: [], total: null }
      out.push(day)
    }
    day.rows.push(row)
    if (row.value_usd) day.total = (day.total ?? 0) + Number(row.value_usd)
  }
  return out
})
const dayLabel = (date: string) => {
  const text = formatDay(`${date}T12:00:00`).replaceAll('.', '')
  return text.charAt(0).toUpperCase() + text.slice(1)
}
const rest = computed(() => props.total - props.rows.length)
const sheetHead = (row: CryptoOpRow) => ({
  title: opTitle(row, t),
  description: shortAddress(row.reference || row.id),
})
</script>

<template>
  <div class="divide-y divide-border">
    <section v-for="day in days" :key="day.date" class="divide-y divide-border">
      <!-- Opaque, no backdrop-blur: a blur on every day strip froze WebKitGTK. -->
      <h3
        class="sticky top-0 z-[1] flex items-baseline justify-between gap-2 py-1.5 text-xs text-muted-foreground"
        :class="phone ? 'border-b-0 bg-card pt-3' : '-mx-4 bg-muted px-4'"
      >
        <span>{{ dayLabel(day.date) }}</span>
        <span v-if="day.total !== null" class="tabular-nums">
          <Money :amount="day.total" currency="usd" :decimals="2" signed />
        </span>
      </h3>
      <CryptoTxRow v-for="row in day.rows" :key="row.id" :row="row" openable @open="emit('open', $event)">
        <template v-if="props.rowActions" #actions="{ row: current }">
          <ActionMenu :actions="props.rowActions(current)" :label="t('actions.more')" :sheet="sheetHead(current)" />
        </template>
      </CryptoTxRow>
    </section>

    <div v-if="rest > 0" class="flex justify-center py-3">
      <Button variant="outline" size="sm" :disabled="props.loadingMore" data-crypto-op-more @click="emit('more')">
        {{ t('ui.showMore', { count: Math.min(rest, CRYPTO_OPS_PAGE) }) }}
      </Button>
    </div>
  </div>
</template>
