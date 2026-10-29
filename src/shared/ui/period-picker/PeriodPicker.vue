<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { CalendarDays, ChevronDown, Check } from 'lucide-vue-next'

import { formatMonthKey } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { type MonthRange, PERIOD_PRESETS, type PeriodPreset, currentMonth, presetOf, presetRange, stepRange } from '@/shared/lib/period'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { Drawer, DrawerContent, DrawerTitle } from '@/shared/ui/drawer'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'

import MonthPanel from './MonthPanel.vue'
import type { PeriodPickerProps } from './period-picker.types'

/**
 * The period chip of a section: ‹ `October 2026` ›. The arrows step by the
 * period's own length; the chip opens the quick periods and the month grid (a
 * popover on a computer, a bottom sheet on a phone). The grid edits a draft,
 * `Done` applies it; a quick period applies at once. On a phone the chip
 * alone, 40px tall with a calendar mark (Money 1 · 1): the sheet picks the period.
 */
const props = defineProps<PeriodPickerProps>()
const range = defineModel<MonthRange>({ required: true })

const { t } = useI18n()
const phone = useIsPhone()
const open = ref(false)
const draft = ref<MonthRange>({ ...range.value })
watch(open, (value) => {
  if (value) draft.value = { ...range.value }
})

const capital = (text: string) => text.charAt(0).toUpperCase() + text.slice(1)
const short = (key: string) => formatMonthKey(key, { month: 'short', year: 'numeric' })
/** `October 2026`, a quick period's name, or `Nov 2025 — Oct 2026`. */
function label(value: MonthRange): string {
  if (value.from === value.to) return capital(formatMonthKey(value.from, { month: 'long', year: 'numeric' }).replace(/\s*г\.$/, ''))
  const preset = presetOf(value)
  if (preset) return t(`period.preset.${preset}`)
  return `${short(value.from)} — ${short(value.to)}`
}
const active = computed(() => presetOf(open.value ? draft.value : range.value))
const atEnd = computed(() => range.value.to >= currentMonth())

function choose(preset: PeriodPreset) {
  range.value = presetRange(preset)
  open.value = false
}
function apply() {
  range.value = { ...draft.value }
  open.value = false
}
function reset() {
  draft.value = { ...props.defaultRange }
}
const TRIGGER =
  'flex h-9 min-w-0 items-center gap-2 rounded-md border border-input bg-background px-3 text-sm shadow-xs data-[state=open]:ring-2 data-[state=open]:ring-ring/40'
</script>

<template>
  <div class="flex min-w-0 items-center gap-2" data-period-picker>
    <Button v-if="!phone" variant="outline" size="icon-sm" :aria-label="t('period.prev')" @click="range = stepRange(range, -1)">‹</Button>
    <Popover v-if="!phone" v-model:open="open">
      <PopoverTrigger as-child>
        <button
          type="button"
          :class="cn(TRIGGER, props.triggerClass ?? 'w-[170px]')"
          data-period-trigger
        >
          <span class="min-w-0 flex-1 truncate text-left">{{ label(range) }}</span>
          <ChevronDown class="size-4 shrink-0 opacity-60" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" :collision-padding="16" class="flex w-[520px] max-w-(--reka-popover-content-available-width) rounded-[10px] p-0" data-period-popover>
        <div class="flex w-[190px] shrink-0 flex-col border-r border-border p-1.5">
          <button
            v-for="preset in PERIOD_PRESETS"
            :key="preset"
            type="button"
            :class="cn('flex items-center justify-between rounded-md px-2.5 py-2 text-left text-sm hover:bg-accent', active === preset && 'bg-muted')"
            @click="choose(preset)"
          >
            <span>{{ t(`period.preset.${preset}`) }}</span>
            <Check v-if="active === preset" class="size-4" />
          </button>
        </div>
        <div class="grid min-w-0 flex-1 gap-3 px-3.5 py-3">
          <MonthPanel v-model="draft" />
          <div class="flex items-center gap-2 border-t border-border pt-2.5 text-sm">
            <Button variant="ghost" size="sm" class="text-muted-foreground" @click="reset">{{ t('period.reset') }}</Button>
            <span class="flex-1" />
            <Button size="sm" @click="apply">{{ t('period.done') }}</Button>
          </div>
        </div>
      </PopoverContent>
    </Popover>
    <template v-else>
      <button type="button" :class="cn(TRIGGER, 'h-10')" data-period-trigger @click="open = true">
        <CalendarDays class="size-4 shrink-0 text-muted-foreground" />
        <span class="truncate">{{ label(range) }}</span>
      </button>
      <Drawer v-model:open="open">
        <DrawerContent class="gap-3 px-4 pt-2 pb-5" data-period-popover>
          <div class="flex items-center">
            <DrawerTitle class="flex-1 text-[17px] font-semibold">{{ t('period.title') }}</DrawerTitle>
            <Button variant="link" size="sm" class="px-0" @click="reset">{{ t('period.reset') }}</Button>
          </div>
          <div class="flex flex-wrap gap-1.5">
            <button
              v-for="preset in PERIOD_PRESETS"
              :key="preset"
              type="button"
              :class="cn('h-8 rounded-full border px-3 text-sm', active === preset ? 'border-primary bg-primary text-primary-foreground' : 'border-border')"
              @click="choose(preset)"
            >
              {{ t(`period.preset.${preset}`) }}
            </button>
          </div>
          <MonthPanel v-model="draft" touch />
          <Button class="h-11" @click="apply">{{ t('period.show', { period: label(draft) }) }}</Button>
        </DrawerContent>
      </Drawer>
    </template>
    <Button v-if="!phone" variant="outline" size="icon-sm" :aria-label="t('period.next')" :disabled="atEnd" @click="range = stepRange(range, 1)">›</Button>
  </div>
</template>
