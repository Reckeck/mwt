<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AmountInput } from '@/shared/ui/amount-input'
import { Checkbox } from '@/shared/ui/checkbox'
import { currencySymbol } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import { lineCalc, lineNote, readInCard } from './model/line-note'
import { priceRow } from './model/price-row'
import type { BillRow } from './utility-bill.types'

/**
 * One line of the bill (Wallets-2 · 13): the tick, the name with what it is
 * made of, a field per zone for a meter not read yet (the amount follows),
 * and the amount on the right — editable, though it reads as plain text.
 */
const props = defineProps<{ row: BillRow; automation: string | null }>()
const { t } = useI18n()

const item = computed(() => props.row.line.item)
const metered = computed(() => ('metered' in item.value.kind ? item.value.kind.metered : null))
const fields = computed(() => {
  if (!metered.value || readInCard(props.row)) return []
  return metered.value.zones.length
    ? metered.value.zones.map((zone) => ({ zone, label: zone }))
    : [{ zone: null, label: t('utilityBill.reading') }]
})
const note = computed(() => lineNote(props.row, t, props.automation))
const calc = computed(() => lineCalc(props.row, t))
const placeholder = (zone: string | null) =>
  (zone === null
    ? props.row.line.previous_reading
    : props.row.line.zones.find((entry) => entry.zone === zone)?.previous_reading) ?? ''

function onReading(value: string | number, zone: string | null) {
  if (zone === null) props.row.reading = String(value)
  else props.row.zoneReadings[zone] = String(value)
  void priceRow(props.row)
}
const FIELD = 'h-8 min-w-0 flex-1 [&_input]:px-2 [&_input]:text-right'
const AMOUNT =
  '-mt-1.5 h-8 w-32 shrink-0 max-md:w-24 max-md:[&_input]:ps-0 border-transparent shadow-none hover:border-input focus-within:border-input [&_input]:pe-1 [&_input]:text-right [&_input]:font-medium'
</script>

<template>
  <li :class="cn('flex items-start gap-3 border-b border-border py-2.5', !props.row.include && 'opacity-55')" data-bill-line>
    <Checkbox v-model="props.row.include" class="mt-0.5" :aria-label="t('utilityBill.include', { name: item.name })" />
    <div class="flex min-w-0 flex-1 flex-col gap-1">
      <span class="text-sm font-medium">{{ item.name }}</span>
      <span class="text-xs text-muted-foreground tabular-nums">{{ note }}</span>
      <div v-if="fields.length && props.row.include" class="flex gap-2">
        <AmountInput
          v-for="field in fields"
          :key="field.label"
          :model-value="field.zone === null ? props.row.reading : (props.row.zoneReadings[field.zone] ?? '')"
          :class="FIELD"
          :placeholder="placeholder(field.zone)"
          :aria-label="`${item.name}: ${field.label}`"
          @update:model-value="(value) => onReading(value, field.zone)"
        >
          <template #prefix><span class="text-xs text-muted-foreground">{{ field.label }}</span></template>
        </AmountInput>
      </div>
      <span v-if="calc && props.row.include" class="text-xs text-muted-foreground tabular-nums" data-bill-calc>{{ calc }}</span>
    </div>
    <span v-if="!props.row.include" class="w-32 shrink-0 pe-3 max-md:w-24 text-right text-sm font-medium">—</span>
    <AmountInput v-else v-model="props.row.amount" :decimals="2" :class="AMOUNT" placeholder="—" :aria-label="t('utilityBill.amountOf', { name: item.name })">
      <template #suffix><span class="pe-2 text-sm font-medium">{{ currencySymbol(item.currency) }}</span></template>
    </AmountInput>
  </li>
</template>
