<script setup lang="ts">
import { Input } from '@/shared/ui/input'
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from '@/shared/ui/sheet'

/**
 * A bottom sheet for choosing one item: a title, a search field, then the list in the default
 * slot. The sheet is at most 85dvh tall and the list scrolls inside it, so the search field and
 * the sheet's edge stay reachable with the keyboard up. The trigger goes in the `trigger` slot.
 */
const props = defineProps<{ title: string; searchLabel: string; overlay: string }>()
const open = defineModel<boolean>('open', { default: false })
const search = defineModel<string>('search', { default: '' })
</script>

<template>
  <Sheet v-model:open="open">
    <SheetTrigger as-child><slot name="trigger" /></SheetTrigger>
    <SheetContent side="bottom" :overlay="props.overlay" class="max-h-[85dvh] gap-3 px-4 pt-4" :aria-describedby="undefined">
      <SheetHeader class="p-0">
        <SheetTitle>{{ props.title }}</SheetTitle>
      </SheetHeader>
      <Input v-model="search" type="search" :aria-label="props.searchLabel" :placeholder="props.searchLabel" autocomplete="off" />
      <div class="min-h-0 flex-1 overflow-y-auto pb-2">
        <slot />
      </div>
    </SheetContent>
  </Sheet>
</template>
