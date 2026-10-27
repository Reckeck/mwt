<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatMonthKey } from '@/shared/lib/format'
import { type MonthRange, currentMonth, monthKey, pickMonth, rangeDays } from '@/shared/lib/period'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

/**
 * The year and its twelve months: a click picks a month, Shift + click (or a
 * second tap on a phone, `extend`) stretches the period to it.
 */
const props = defineProps<{ touch?: boolean }>()
const range = defineModel<MonthRange>({ required: true })

const { t } = useI18n()
const year = ref(Number(range.value.to.slice(0, 4)))
watch(
  () => range.value.to,
  (to) => {
    year.value = Number(to.slice(0, 4))
  },
)
const now = currentMonth()
const months = computed(() => Array.from({ length: 12 }, (_unused, index) => monthKey(year.value, index)))
/** On a phone a second tap extends the period from the first; there is no Shift. */
const anchor = ref<string | null>(null)

function pick(key: string, event: MouseEvent) {
  const extend = event.shiftKey || (props.touch && anchor.value !== null)
  range.value = pickMonth(extend && anchor.value ? { from: anchor.value, to: anchor.value } : range.value, key, extend)
  anchor.value = extend ? null : key
}

function tone(key: string) {
  if (key > now) return 'text-muted-foreground opacity-40'
  if (key === range.value.from || key === range.value.to) return 'bg-primary font-semibold text-primary-foreground border-primary'
  if (key > range.value.from && key < range.value.to) return 'bg-primary/15 border-primary/30'
  return 'border-border hover:bg-accent'
}
const days = computed(() => rangeDays(range.value))
const dayText = (iso: string) => iso.split('-').reverse().join('.')
</script>

<template>
  <div class="grid gap-3">
    <div class="flex items-center gap-2">
      <Button variant="outline" size="icon-sm" class="size-7" :aria-label="t('period.prevYear')" @click="year -= 1">‹</Button>
      <b class="flex-1 text-center text-sm">{{ year }}</b>
      <Button variant="outline" size="icon-sm" class="size-7" :aria-label="t('period.nextYear')" :disabled="year >= Number(now.slice(0, 4))" @click="year += 1">›</Button>
    </div>
    <div class="grid grid-cols-4 gap-1">
      <button
        v-for="key in months"
        :key="key"
        type="button"
        :disabled="key > now"
        :aria-pressed="key >= range.from && key <= range.to"
        :class="cn('flex items-center justify-center rounded-[7px] border', props.touch ? 'h-10 text-sm' : 'h-[34px] text-[13px]', tone(key))"
        @click="pick(key, $event)"
      >
        {{ formatMonthKey(key, { month: 'short' }).replace('.', '') }}
      </button>
    </div>
    <span v-if="!props.touch" class="text-xs text-muted-foreground">{{ t('period.hint') }}</span>
    <div class="flex items-center gap-2 text-sm">
      <span v-if="!props.touch" class="text-muted-foreground">{{ t('period.from') }}</span>
      <span class="flex h-8 flex-1 items-center rounded-md border border-input px-3 tabular-nums" data-period-from>
        {{ props.touch ? `${t('period.from')} ${dayText(days.first)}` : dayText(days.first) }}
      </span>
      <span v-if="!props.touch" class="text-muted-foreground">{{ t('period.to') }}</span>
      <span class="flex h-8 flex-1 items-center rounded-md border border-input px-3 tabular-nums" data-period-to>
        {{ props.touch ? `${t('period.to')} ${dayText(days.last)}` : dayText(days.last) }}
      </span>
    </div>
  </div>
</template>
