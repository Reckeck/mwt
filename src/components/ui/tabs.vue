<script setup lang="ts">
import { TabsContent, TabsList, TabsRoot, TabsTrigger } from 'reka-ui'

export type TabDef = { value: string; label: string }

const props = defineProps<{
  modelValue: string
  tabs: TabDef[]
}>()

const emit = defineEmits<{ 'update:modelValue': [string] }>()

defineSlots<Record<string, () => unknown>>()
</script>

<template>
  <TabsRoot
    :model-value="props.modelValue"
    @update:model-value="(v) => emit('update:modelValue', String(v))"
  >
    <TabsList class="inline-flex items-center gap-1 rounded-lg bg-muted p-1">
      <TabsTrigger
        v-for="tab in props.tabs"
        :key="tab.value"
        :value="tab.value"
        class="rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow"
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
