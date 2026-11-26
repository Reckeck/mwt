<script setup lang="ts">
import { computed, ref, useAttrs } from 'vue'
import { useI18n } from 'vue-i18n'
import { CalendarDays } from 'lucide-vue-next'

import { activeLocale } from '@/shared/lib/locale'
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from '@/shared/ui/sheet'

import type { DateTimePickerProps } from './date-time-picker.types'
import DateTimePanel from './DateTimePanel.vue'
import { formatValue } from './date-value'

/**
 * A date, a date with time, or a month, picked from a calendar instead of the browser's own input
 * (which looks different on every system and ignores the app's language): a popover on a wide
 * screen, a bottom sheet on a phone. The model is the same string a native
 * `type="date|datetime-local|month"` input holds, so it drops in for one.
 * `id`, `aria-*` and `class` go to the trigger.
 */
defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<DateTimePickerProps>(), {
  mode: 'date',
  placeholder: undefined,
  chip: false,
  min: undefined,
  max: undefined,
})
const model = defineModel<string | null>({ default: '' })

const { t } = useI18n()
const attrs = useAttrs()
const open = ref(false)
const phone = useIsPhone()

const shown = computed(() => formatValue(props.mode, model.value ?? '', activeLocale.value))
const triggerClass = computed(() =>
  cn(
    props.chip ? 'h-9 w-auto max-w-full justify-start gap-2 rounded-full px-3 font-normal' : 'w-full justify-start gap-2 px-3 font-normal',
    !shown.value && 'text-muted-foreground',
    props.invalid && 'border-destructive',
    attrs.class as string,
  ),
)
</script>

<template>
  <Sheet v-if="phone" v-model:open="open">
    <SheetTrigger as-child>
      <Button variant="outline" :disabled="props.disabled" :aria-invalid="props.invalid || undefined" v-bind="attrs" :class="triggerClass">
        <CalendarDays class="size-4 shrink-0 text-muted-foreground" />
        <span class="truncate">{{ shown || props.placeholder || t(`ui.pick.${props.mode}`) }}</span>
      </Button>
    </SheetTrigger>
    <SheetContent side="bottom" overlay="date-time-picker" class="max-h-[85dvh] gap-3 px-4 pt-4" :aria-describedby="undefined">
      <SheetHeader class="p-0">
        <SheetTitle>{{ t(`ui.pick.${props.mode}`) }}</SheetTitle>
      </SheetHeader>
      <DateTimePanel
        v-model="model"
        class="min-h-0 flex-1 overflow-y-auto"
        :mode="props.mode"
        :required="props.required"
        :min="props.min"
        :max="props.max"
        :time-id="String(attrs.id ?? 'dt')"
        @close="open = false"
      />
    </SheetContent>
  </Sheet>
  <Popover v-else v-model:open="open">
    <PopoverTrigger as-child>
      <Button variant="outline" :disabled="props.disabled" :aria-invalid="props.invalid || undefined" v-bind="attrs" :class="triggerClass">
        <CalendarDays class="size-4 shrink-0 text-muted-foreground" />
        <span class="truncate">{{ shown || props.placeholder || t(`ui.pick.${props.mode}`) }}</span>
      </Button>
    </PopoverTrigger>
    <PopoverContent class="w-auto p-0" align="start">
      <DateTimePanel
        v-model="model"
        :mode="props.mode"
        :required="props.required"
        :min="props.min"
        :max="props.max"
        :time-id="String(attrs.id ?? 'dt')"
        @close="open = false"
      />
    </PopoverContent>
  </Popover>
</template>
