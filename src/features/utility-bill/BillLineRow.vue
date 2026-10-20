<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { Checkbox } from '@/shared/ui/checkbox'

import { priceRow, rowConsumption } from './model/price-row'
import type { BillRow } from './utility-bill.types'

/** One bill line: a meter takes its reading (one per zone) and re-prices itself. */
const props = defineProps<{ row: BillRow }>()

const { t } = useI18n()

const item = computed(() => props.row.line.item)
const metered = computed(() => ('metered' in item.value.kind ? item.value.kind.metered : null))
const consumption = computed(() => rowConsumption(props.row))
const typed = computed(() =>
  metered.value?.zones.length ? Object.values(props.row.zoneReadings).some(Boolean) : Boolean(props.row.reading),
)
const previous = computed(() =>
  metered.value?.zones.length
    ? props.row.line.zones.map((zone) => `${zone.zone} ${zone.previous_reading ?? '—'}`).join(' · ')
    : (props.row.line.previous_reading ?? '—'),
)

function onReading(value: string | number, zone: string | null) {
  if (zone === null) props.row.reading = String(value)
  else props.row.zoneReadings[zone] = String(value)
  void priceRow(props.row)
}
</script>

<template>
  <li class="grid items-center gap-2 p-2.5 @xl:grid-cols-[1.5rem_1fr_9rem_9rem]">
    <Checkbox v-model="props.row.include" :aria-label="t('utilityBill.include', { name: item.name })" />
    <div class="min-w-0">
      <p class="flex items-center gap-2 truncate text-sm font-medium">
        {{ item.name }}
        <Badge v-if="Number(props.row.line.paid) > 0" variant="secondary">
          {{ t('utilityBill.alreadyPaid') }} <Money :amount="props.row.line.paid" :currency="item.currency" :convert="false" />
        </Badge>
      </p>
      <p v-if="metered" class="text-xs text-muted-foreground tabular-nums">
        {{ t('utilityBill.previous', { value: previous, unit: metered.unit }) }}
        <template v-if="consumption !== null">· {{ t('utilities.consumption') }}: {{ consumption }} {{ metered.unit }}</template>
      </p>
      <p v-if="metered && !typed" class="text-xs text-muted-foreground">{{ t('utilityBill.enterReading') }}</p>
      <p v-else-if="props.row.line.includes_deposit" class="text-xs text-muted-foreground">{{ t('utilities.depositIncluded') }}</p>
    </div>
    <div v-if="metered?.zones.length" class="grid gap-1">
      <AmountInput
        v-for="zone in metered.zones"
        :key="zone"
        :model-value="props.row.zoneReadings[zone] ?? ''"
        :placeholder="zone"
        :aria-label="`${t('utilities.reading')}: ${zone}`"
        @update:model-value="(value) => onReading(value, zone)"
      />
    </div>
    <AmountInput
      v-else-if="metered"
      :model-value="props.row.reading"
      :placeholder="t('utilities.reading')"
      :aria-label="t('utilities.reading')"
      @update:model-value="(value) => onReading(value, null)"
    />
    <span v-else class="hidden @xl:block" />
    <AmountInput v-model="props.row.amount" :decimals="2" :aria-label="t('utilities.amount')">
      <template #suffix>
        <span class="px-2 text-xs uppercase text-muted-foreground">{{ item.currency }}</span>
      </template>
    </AmountInput>
  </li>
</template>
