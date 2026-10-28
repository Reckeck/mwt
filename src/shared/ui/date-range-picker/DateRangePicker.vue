<script setup lang="ts">
import { CalendarDays } from 'lucide-vue-next'
import { computed, ref, useAttrs } from 'vue'
import { useI18n } from 'vue-i18n'

import { type DateRange, formatDateRange } from '@/shared/lib/date-range'
import { activeLocale } from '@/shared/lib/locale'
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { Drawer, DrawerContent, DrawerTitle } from '@/shared/ui/drawer'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'

import DateRangePickerPanel from './DateRangePickerPanel.vue'
import type { DateRangePickerProps } from './date-range-picker.types'

/**
 * One day or a span of days («2 ноя», «2–4 ноя», «30 окт – 2 ноя») picked from a
 * calendar: a popover on a computer, a bottom sheet on a phone. The model is
 * `{ start, end }` as `YYYY-MM-DD` (`end` empty for one day) or null for none.
 * `id`, `aria-*` and `class` go to the trigger.
 */
defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<DateRangePickerProps>(), { placeholder: undefined })
const model = defineModel<DateRange | null>({ default: null })

const { t } = useI18n()
const attrs = useAttrs()
const phone = useIsPhone()
const open = ref(false)

const shown = computed(() => formatDateRange(model.value, activeLocale.value))
const hint = computed(() => props.placeholder || t(props.range ? 'ui.pick.dateRange' : 'ui.pick.date'))
const TRIGGER = 'w-full justify-start gap-2 px-3 font-normal'
</script>

<template>
  <Popover v-if="!phone" v-model:open="open">
    <PopoverTrigger as-child>
      <Button
        variant="outline"
        :disabled="props.disabled"
        :aria-invalid="props.invalid || undefined"
        v-bind="attrs"
        :class="cn(TRIGGER, !shown && 'text-muted-foreground', props.invalid && 'border-destructive', attrs.class as string)"
      >
        <CalendarDays class="size-4 shrink-0 text-muted-foreground" />
        <span class="truncate">{{ shown || hint }}</span>
      </Button>
    </PopoverTrigger>
    <PopoverContent class="w-auto p-0" align="start">
      <DateRangePickerPanel v-model="model" :range="props.range ?? false" @done="open = false" />
    </PopoverContent>
  </Popover>
  <template v-else>
    <Button
      variant="outline"
      :disabled="props.disabled"
      :aria-invalid="props.invalid || undefined"
      v-bind="attrs"
      :class="cn(TRIGGER, !shown && 'text-muted-foreground', props.invalid && 'border-destructive', attrs.class as string)"
      @click="open = true"
    >
      <CalendarDays class="size-4 shrink-0 text-muted-foreground" />
      <span class="truncate">{{ shown || hint }}</span>
    </Button>
    <Drawer v-model:open="open">
      <DrawerContent class="gap-1 px-2 pt-2 pb-5">
        <DrawerTitle class="px-2 text-[17px] font-semibold">{{ hint }}</DrawerTitle>
        <DateRangePickerPanel v-model="model" :range="props.range ?? false" @done="open = false" />
      </DrawerContent>
    </Drawer>
  </template>
</template>
