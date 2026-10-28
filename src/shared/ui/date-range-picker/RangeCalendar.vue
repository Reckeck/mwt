<script setup lang="ts">
import type { DateRange as CalendarRange } from 'reka-ui'
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'
import {
  RangeCalendarCell,
  RangeCalendarCellTrigger,
  RangeCalendarGrid,
  RangeCalendarGridBody,
  RangeCalendarGridHead,
  RangeCalendarGridRow,
  RangeCalendarHeadCell,
  RangeCalendarHeader,
  RangeCalendarHeading,
  RangeCalendarNext,
  RangeCalendarPrev,
  RangeCalendarRoot,
} from 'reka-ui'

import { buttonVariants } from '@/shared/ui/button'
import { cn } from '@/shared/lib/utils'

/** The calendar of a span of days: the shadcn look of `Calendar` on reka's range parts. */
defineProps<{ locale: string; weekStartsOn: 0 | 1 }>()
const model = defineModel<CalendarRange>({ required: true })

const CELL = cn(
  'relative flex-1 p-0 text-center text-sm [&:has([data-selected])]:bg-accent',
  'first:[&:has([data-selected])]:rounded-l-md last:[&:has([data-selected])]:rounded-r-md',
)
const NAV = cn(buttonVariants({ variant: 'outline' }), 'size-7 bg-transparent p-0 opacity-60 hover:opacity-100')
</script>

<template>
  <RangeCalendarRoot
    v-slot="{ grid, weekDays }"
    v-model="model"
    :locale="locale"
    :week-starts-on="weekStartsOn"
    fixed-weeks
    class="p-3"
    data-slot="range-calendar"
  >
    <RangeCalendarHeader class="relative flex items-center justify-between">
      <RangeCalendarPrev :class="NAV" :aria-label="'‹'"><ChevronLeft class="size-4" /></RangeCalendarPrev>
      <RangeCalendarHeading class="text-sm font-medium first-letter:uppercase" />
      <RangeCalendarNext :class="NAV" :aria-label="'›'"><ChevronRight class="size-4" /></RangeCalendarNext>
    </RangeCalendarHeader>
    <RangeCalendarGrid v-for="month in grid" :key="month.value.toString()" class="mt-3 w-full border-collapse">
      <RangeCalendarGridHead>
        <RangeCalendarGridRow class="flex">
          <RangeCalendarHeadCell v-for="day in weekDays" :key="day" class="flex-1 text-[0.8rem] font-normal text-muted-foreground">
            {{ day }}
          </RangeCalendarHeadCell>
        </RangeCalendarGridRow>
      </RangeCalendarGridHead>
      <RangeCalendarGridBody>
        <RangeCalendarGridRow v-for="(week, index) in month.rows" :key="index" class="mt-1 flex w-full">
          <RangeCalendarCell
            v-for="day in week"
            :key="day.toString()"
            :date="day"
            :class="CELL"
          >
            <RangeCalendarCellTrigger
              :day="day"
              :month="month.value"
              :class="
                cn(
                  buttonVariants({ variant: 'ghost' }),
                  'size-9 p-0 font-normal md:size-8',
                  'data-[outside-view]:text-muted-foreground data-[disabled]:opacity-50',
                  '[&[data-today]:not([data-selected])]:bg-accent',
                  'data-[selection-start]:bg-primary data-[selection-start]:text-primary-foreground',
                  'data-[selection-end]:bg-primary data-[selection-end]:text-primary-foreground',
                )
              "
            />
          </RangeCalendarCell>
        </RangeCalendarGridRow>
      </RangeCalendarGridBody>
    </RangeCalendarGrid>
  </RangeCalendarRoot>
</template>
