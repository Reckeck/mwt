<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { X } from 'lucide-vue-next'

import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { Sheet, SheetClose, SheetContent, SheetFooter, SheetHeader, SheetTitle, SheetTrigger } from '@/shared/ui/sheet'

/**
 * A panel opened from a button: a bottom sheet on a phone (rule 16), a popover next to the trigger
 * on a desktop, so a short list or a few fields do not stretch across the whole screen. `header`
 * goes above the scrolling body (a search field), `footer` below it. The body keeps a 4 px inset
 * so focus rings of its fields are not clipped by the scroller.
 * `surface="picker"` is the Picker of board 1097003: a sheet with a grabber, a 20/28 title and a
 * 44 px close button (boards 532847, 532506); a popover with a 6 px inset, radius 12 and the
 * popover shadow, its rows edge to edge.
 */
const props = withDefaults(
  defineProps<{
    title: string
    overlay: string
    width?: string
    showTitle?: boolean
    align?: 'start' | 'center' | 'end'
    surface?: 'panel' | 'picker'
  }>(),
  { width: 'w-96', showTitle: false, align: 'start', surface: 'panel' },
)
const open = defineModel<boolean>('open', { default: false })
const phone = useIsPhone()
const { t } = useI18n()
const picker = computed(() => props.surface === 'picker')
</script>

<template>
  <Sheet v-if="phone" v-model:open="open">
    <SheetTrigger as-child><slot name="trigger" /></SheetTrigger>
    <SheetContent
      side="bottom"
      :overlay="props.overlay"
      :hide-close="picker"
      :class="picker ? 'max-h-[85dvh] gap-3 rounded-t-xl px-4 pt-2 pb-[calc(1.75rem+var(--safe-bottom))]' : 'max-h-[85dvh] gap-3 px-4 pt-4'"
      :aria-describedby="undefined"
    >
      <template v-if="picker">
        <span class="mx-auto h-1 w-9 shrink-0 rounded-full bg-strong" aria-hidden="true" />
        <SheetHeader class="flex-row items-center justify-between gap-2 p-0">
          <SheetTitle class="text-xl font-semibold">{{ props.title }}</SheetTitle>
          <SheetClose class="-mr-2.5 grid size-11 shrink-0 place-items-center rounded-full text-muted-foreground hover:bg-hover">
            <X class="size-5" aria-hidden="true" />
            <span class="sr-only">{{ t('actions.close') }}</span>
          </SheetClose>
        </SheetHeader>
      </template>
      <SheetHeader v-else class="p-0">
        <SheetTitle>{{ props.title }}</SheetTitle>
      </SheetHeader>
      <slot name="header" />
      <div :class="picker ? 'min-h-0 flex-1 overflow-y-auto' : '-mx-1 min-h-0 flex-1 overflow-y-auto px-1 pb-2'">
        <slot />
      </div>
      <SheetFooter v-if="$slots.footer" class="p-0 pb-2">
        <slot name="footer" />
      </SheetFooter>
    </SheetContent>
  </Sheet>
  <Popover v-else v-model:open="open">
    <PopoverTrigger as-child><slot name="trigger" /></PopoverTrigger>
    <PopoverContent
      :overlay="props.overlay"
      :align="props.align"
      :aria-label="props.title"
      :class="
        cn(
          'flex max-h-[min(32rem,var(--reka-popover-content-available-height))] flex-col overflow-hidden',
          picker ? 'max-h-[min(23.25rem,var(--reka-popover-content-available-height))] gap-0.5 rounded-md p-1.5 shadow-popover' : 'gap-2 p-2',
          props.width,
        )
      "
    >
      <p v-if="props.showTitle" class="px-1 pt-1 text-sm font-semibold">{{ props.title }}</p>
      <slot name="header" />
      <div :class="picker ? 'min-h-0 flex-1 overflow-y-auto [scrollbar-gutter:stable]' : 'min-h-0 flex-1 overflow-y-auto p-1 [scrollbar-gutter:stable]'">
        <slot />
      </div>
      <div v-if="$slots.footer" class="flex justify-end gap-2 px-1 pb-1">
        <slot name="footer" />
      </div>
    </PopoverContent>
  </Popover>
</template>
