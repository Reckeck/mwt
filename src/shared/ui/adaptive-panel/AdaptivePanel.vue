<script setup lang="ts">
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { Sheet, SheetContent, SheetFooter, SheetHeader, SheetTitle, SheetTrigger } from '@/shared/ui/sheet'

/**
 * A panel opened from a button: a bottom sheet on a phone (rule 16), a popover next to the trigger
 * on a desktop, so a short list or a few fields do not stretch across the whole screen. `header`
 * goes above the scrolling body (a search field), `footer` below it. The body keeps a 4 px inset
 * so focus rings of its fields are not clipped by the scroller.
 */
const props = withDefaults(
  defineProps<{ title: string; overlay: string; width?: string; showTitle?: boolean; align?: 'start' | 'center' | 'end' }>(),
  { width: 'w-96', showTitle: false, align: 'start' },
)
const open = defineModel<boolean>('open', { default: false })
const phone = useIsPhone()
</script>

<template>
  <Sheet v-if="phone" v-model:open="open">
    <SheetTrigger as-child><slot name="trigger" /></SheetTrigger>
    <SheetContent side="bottom" :overlay="props.overlay" class="max-h-[85dvh] gap-3 px-4 pt-4" :aria-describedby="undefined">
      <SheetHeader class="p-0">
        <SheetTitle>{{ props.title }}</SheetTitle>
      </SheetHeader>
      <slot name="header" />
      <div class="-mx-1 min-h-0 flex-1 overflow-y-auto px-1 pb-2">
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
      :class="cn('flex max-h-[min(32rem,var(--reka-popover-content-available-height))] flex-col gap-2 overflow-hidden p-2', props.width)"
    >
      <p v-if="props.showTitle" class="px-1 pt-1 text-sm font-semibold">{{ props.title }}</p>
      <slot name="header" />
      <div class="min-h-0 flex-1 overflow-y-auto p-1 [scrollbar-gutter:stable]">
        <slot />
      </div>
      <div v-if="$slots.footer" class="flex justify-end gap-2 px-1 pb-1">
        <slot name="footer" />
      </div>
    </PopoverContent>
  </Popover>
</template>
