<script setup lang="ts">
import type { Component } from 'vue'
import { ChevronRight } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { cn } from '@/shared/lib/utils'

/**
 * A choice of the wizard (Auto-1 · 6–7): a card with a title and a hint, the
 * picked one outlined in primary; `row` is the phone's list line with a chevron.
 */
const props = defineProps<{ title: string; hint: string; icon?: Component; selected?: boolean; row?: boolean }>()

const CARD = 'h-auto items-start justify-start gap-2 whitespace-normal rounded-[10px] p-3 text-left font-normal'
const ROW = [
  'h-auto min-h-13 w-full items-center justify-start gap-3 whitespace-normal rounded-none',
  'border-0 border-b px-0 py-2 text-left font-normal shadow-none hover:bg-transparent',
].join(' ')
</script>

<template>
  <Button
    variant="outline"
    :aria-pressed="props.row ? undefined : props.selected"
    :class="cn(props.row ? ROW : CARD, !props.row && props.selected && 'border-primary ring-1 ring-primary hover:bg-background')"
  >
    <component :is="props.icon" v-if="props.icon" :class="cn('shrink-0', props.row ? 'size-5 text-muted-foreground' : 'mt-0.5 size-4')" />
    <span class="grid min-w-0 flex-1 gap-1" :class="props.row && 'gap-0'">
      <span class="text-sm font-medium">{{ props.title }}</span>
      <span class="text-xs text-muted-foreground">{{ props.hint }}</span>
    </span>
    <ChevronRight v-if="props.row" class="size-4 shrink-0 text-muted-foreground" />
  </Button>
</template>
