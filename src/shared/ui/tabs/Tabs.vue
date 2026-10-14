<script setup lang="ts">
import { TabsContent, TabsList, TabsRoot, TabsTrigger } from 'reka-ui'

import type { TabsProps } from './tabs.types'

const props = defineProps<TabsProps>()

const emit = defineEmits<{ 'update:modelValue': [string] }>()

defineSlots<Record<string, () => unknown>>()
</script>

<template>
  <TabsRoot
    :model-value="props.modelValue"
    @update:model-value="(value) => emit('update:modelValue', String(value))"
  >
    <TabsList class="inline-flex max-w-full items-center gap-1 overflow-x-auto rounded-lg bg-muted p-1">
      <TabsTrigger
        v-for="tab in props.tabs"
        :key="tab.value"
        :value="tab.value"
        class="shrink-0 whitespace-nowrap rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow"
      >
        {{ tab.label }}
      </TabsTrigger>
    </TabsList>

    <TabsContent
      v-for="tab in props.tabs"
      :key="tab.value"
      :value="tab.value"
      class="mt-4 focus:outline-none"
    >
      <slot :name="tab.value" />
    </TabsContent>
  </TabsRoot>
</template>
