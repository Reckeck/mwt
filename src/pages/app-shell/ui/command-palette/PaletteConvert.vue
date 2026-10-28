<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEventListener } from '@vueuse/core'
import { ArrowLeftRight, Calculator, CalendarDays, Plus } from 'lucide-vue-next'

import { CommandGroup, CommandItem, CommandShortcut, useCommand } from '@/shared/ui/command'
import { openConverter } from '@/shared/lib/converter'
import { currencySymbol, formatDayMonthShort, formatMoney, formatQuantity, isFiatCode } from '@/shared/lib/format'
import { hotkeyLabel } from '@/shared/lib/hotkeys'
import { notify } from '@/shared/lib/notify'
import { openQuickAdd } from '@/shared/lib/quick-add'

import { plainAmount, withTarget } from './convert-query'
import type { usePaletteConvert } from './use-palette-convert'

/**
 * The converter part of Ctrl K (Currency-2 · 10): the typed sum at today's
 * rate, on the day of the last exchange, and what to do with it. Enter copies
 * a row's number, Tab asks for the next favourite, Ctrl Enter records an
 * exchange of the sum.
 */
const props = defineProps<{ convert: ReturnType<typeof usePaletteConvert>; run: (action: () => void) => void }>()

const { t } = useI18n()
const { filterState } = useCommand()

const query = computed(() => props.convert.query.value)
/** «100 $»: the typed sum as typed, without padding zeros. */
const typed = computed(() => (query.value ? `${formatQuantity(query.value.amount)} ${currencySymbol(query.value.from)}` : ''))
const now = computed(() => props.convert.now.value.map((row) => ({ ...row, text: formatMoney(row.value, { currency: row.currency }) })))
const dated = computed(() => {
  const row = props.convert.dated.value
  return row && { ...row, day: formatDayMonthShort(row.at), text: formatMoney(row.value, { currency: row.currency }) }
})

function copy(value: number, currency: string) {
  const text = plainAmount(value, isFiatCode(currency))
  props.run(() => {
    void navigator.clipboard?.writeText(text).catch(() => undefined)
    notify.success('ui.copied')
  })
}
function recordExchange() {
  const asked = query.value
  if (!asked) return
  props.run(() => openQuickAdd({ kind: 'convert', amount: String(asked.amount) }))
}

useEventListener(window, 'keydown', (event: KeyboardEvent) => {
  if (!query.value) return
  if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
    event.preventDefault()
    event.stopPropagation()
    recordExchange()
  } else if (event.key === 'Tab' && !event.shiftKey && !event.altKey) {
    const targets = props.convert.targets.value
    if (!targets.length) return
    event.preventDefault()
    const next = targets[(targets.indexOf(query.value.to) + 1) % targets.length] ?? targets[0]
    filterState.search = withTarget(filterState.search, next ?? '')
  }
})
</script>

<template>
  <template v-if="query">
    <!-- Each row carries the typed text: the list filter would hide a row that does not contain it. -->
    <CommandGroup v-if="now.length" :heading="t('palette.convert.now')" data-palette-convert>
      <CommandItem
        v-for="(row, position) in now"
        :key="row.currency"
        :value="`convert-now-${row.currency}`"
        data-palette-convert-row
        @select="copy(row.value, row.currency)"
      >
        <ArrowLeftRight />
        <span class="min-w-0 flex-1 truncate tabular-nums">{{ typed }} = {{ row.text }}</span>
        <span v-if="position === 0" class="shrink-0 text-xs text-muted-foreground pointer-coarse:hidden">{{ t('palette.convert.copy') }}</span>
        <span class="sr-only">{{ filterState.search }}</span>
      </CommandItem>
    </CommandGroup>
    <CommandGroup v-if="dated" :heading="t('palette.convert.dated')">
      <CommandItem :value="`convert-dated-${dated.currency}`" @select="copy(dated.value, dated.currency)">
        <CalendarDays />
        <span class="min-w-0 flex-1 truncate tabular-nums">{{ t('palette.convert.on', { sum: typed, day: dated.day, value: dated.text }) }}</span>
        <span class="shrink-0 text-xs text-muted-foreground">{{ t('palette.convert.datedNote') }}</span>
        <span class="sr-only">{{ filterState.search }}</span>
      </CommandItem>
    </CommandGroup>
    <CommandGroup :heading="t('palette.actions')">
      <CommandItem value="convert-record" data-palette-convert-record @select="recordExchange">
        <Plus />
        <span class="min-w-0 flex-1 truncate">{{ t('palette.convert.record', { sum: `${formatQuantity(query.amount)} ${query.from.toUpperCase()}` }) }}</span>
        <CommandShortcut class="tracking-normal">{{ hotkeyLabel('saveAndNext') }}</CommandShortcut>
        <span class="sr-only">{{ filterState.search }}</span>
      </CommandItem>
      <CommandItem value="convert-open" @select="props.run(() => openConverter(query?.from ?? null))">
        <Calculator />
        <span class="min-w-0 flex-1 truncate">{{ t('palette.convert.open') }}</span>
        <span class="sr-only">{{ filterState.search }}</span>
      </CommandItem>
    </CommandGroup>
  </template>
</template>
