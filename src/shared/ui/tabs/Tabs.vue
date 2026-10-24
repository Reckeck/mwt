<script setup lang="ts">
import { computed, useSlots } from 'vue'
import { DropdownMenuItem, TabsContent, TabsList, TabsRoot, TabsTrigger } from 'reka-ui'
import { ChevronDown } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { MENU_ITEM_CLASS } from '@/shared/ui/context-menu'
import { cn } from '@/shared/lib/utils'

import type { TabsProps } from './tabs.types'

const props = withDefaults(defineProps<TabsProps>(), { more: () => [], moreLabel: '', fill: false })

const emit = defineEmits<{ 'update:modelValue': [string] }>()

/** One slot per tab value, and `end` for what stands right of the strip. */
defineSlots<Record<string, () => unknown>>()

const openMore = computed(() => props.more.find((tab) => tab.value === props.modelValue) ?? null)
const slots = useSlots()
// Only tabs with their own slot get a pane: a bare strip (content outside) renders none.
const panes = computed(() => [...props.tabs, ...props.more].filter((tab) => slots[tab.value]))

const TAB =
  'flex h-[1.875rem] shrink-0 items-center gap-1 whitespace-nowrap rounded-md px-3 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm'
</script>

<template>
  <TabsRoot
    :model-value="props.modelValue"
    @update:model-value="(value) => emit('update:modelValue', String(value))"
  >
    <div class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
      <TabsList :class="cn('scrollbar-none inline-flex max-w-full items-center gap-0.5 overflow-x-auto rounded-[0.625rem] bg-muted p-[3px]', props.fill && 'w-full')">
        <TabsTrigger v-for="tab in props.tabs" :key="tab.value" :value="tab.value" :class="cn(TAB, props.fill && 'flex-1 justify-center pointer-coarse:h-10')">
          {{ tab.label }}
        </TabsTrigger>
        <DropdownMenu v-if="props.more.length" align="start">
          <template #trigger>
            <Button variant="ghost" :class="cn(TAB, 'touch-hit hover:bg-transparent')" :data-state="openMore ? 'active' : 'inactive'" data-tabs-more>
              {{ openMore?.label ?? props.moreLabel }}
              <ChevronDown class="size-3.5" aria-hidden="true" />
            </Button>
          </template>
          <DropdownMenuItem
            v-for="tab in props.more"
            :key="tab.value"
            :class="cn(MENU_ITEM_CLASS, tab.value === props.modelValue && 'bg-accent')"
            @select="emit('update:modelValue', tab.value)"
          >
            {{ tab.label }}
          </DropdownMenuItem>
        </DropdownMenu>
      </TabsList>
      <slot name="end" />
    </div>

    <TabsContent v-for="tab in panes" :key="tab.value" :value="tab.value" class="mt-4 focus:outline-none">
      <slot :name="tab.value" />
    </TabsContent>
  </TabsRoot>
</template>
