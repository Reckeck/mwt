<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { TabsList, TabsRoot, TabsTrigger } from 'reka-ui'

import type { RouteTabsProps } from './route-tabs.types'

/**
 * Tabs that are child routes: each has its own address, Back returns to the
 * previous tab. On a narrow screen the strip scrolls sideways and keeps the
 * active tab in view.
 */
const props = withDefaults(defineProps<RouteTabsProps>(), { keepQuery: false })

const route = useRoute()
const router = useRouter()
const list = ref<InstanceType<typeof TabsList> | null>(null)

function open(name: string) {
  if (name === route.name) return
  const keep = props.keepQuery
  const query = keep === true ? route.query : typeof keep === 'function' ? keep(route.query) : undefined
  void router.push({ name, query })
}

watch(
  () => route.name,
  async () => {
    await nextTick()
    const strip = list.value?.$el as HTMLElement | undefined
    strip?.querySelector<HTMLElement>('[data-state="active"]')?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  },
  { immediate: true },
)
</script>

<template>
  <TabsRoot :model-value="String(route.name ?? '')" activation-mode="manual" @update:model-value="(value) => open(String(value))">
    <TabsList
      ref="list"
      class="scrollbar-none inline-flex max-w-full items-center gap-1 overflow-x-auto rounded-lg bg-muted p-1"
    >
      <TabsTrigger
        v-for="tab in props.tabs"
        :key="tab.name"
        :value="tab.name"
        class="shrink-0 whitespace-nowrap rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow"
      >
        {{ tab.label }}
      </TabsTrigger>
    </TabsList>
  </TabsRoot>
</template>
