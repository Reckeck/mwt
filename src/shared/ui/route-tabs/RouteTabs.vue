<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { DropdownMenuItem, TabsList, TabsRoot, TabsTrigger } from 'reka-ui'
import { ChevronDown } from 'lucide-vue-next'

import { ActionSheet } from '@/shared/ui/action-menu'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { MENU_ITEM_CLASS } from '@/shared/ui/context-menu'
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'

import type { RouteTabsProps } from './route-tabs.types'

/**
 * Tabs that are child routes: each has its own address, Back returns to the
 * previous tab. On a narrow screen the strip scrolls sideways and keeps the
 * active tab in view. Rare tabs wait behind «More ▾».
 */
const props = withDefaults(defineProps<RouteTabsProps>(), { keepQuery: false, more: () => [], moreLabel: '' })

const route = useRoute()
const router = useRouter()
const phone = useIsPhone()
const sheetOpen = ref(false)
const list = ref<InstanceType<typeof TabsList> | null>(null)

/** The tab a route belongs to: its own name, or `meta.tab` for a screen opened from a tab (an event from the events list). */
const current = computed(() => String(route.meta.tab ?? route.name ?? ''))

function open(name: string) {
  if (name === route.name) return
  const keep = props.keepQuery
  const query = keep === true ? route.query : typeof keep === 'function' ? keep(route.query) : undefined
  void router.push({ name, query })
}

/** The open tab behind «More», if any: the button wears its name. */
const openMore = computed(() => props.more.find((tab) => tab.name === current.value) ?? null)

/** On a phone «More» is a sheet from below, like every other menu there. */
const moreActions = computed(() =>
  props.more.map((tab) => ({ id: tab.name, label: tab.label, hint: tab.hint, opens: true, run: () => open(tab.name) })),
)

watch(
  () => route.name,
  async () => {
    await nextTick()
    const strip = list.value?.$el as HTMLElement | undefined
    strip?.querySelector<HTMLElement>('[data-state="active"]')?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  },
  { immediate: true },
)

const TAB =
  'flex h-[1.875rem] shrink-0 items-center gap-1 whitespace-nowrap rounded-md px-3 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm'
</script>

<template>
  <TabsRoot :model-value="current" activation-mode="manual" @update:model-value="(value) => open(String(value))">
    <TabsList ref="list" class="scrollbar-none inline-flex max-w-full items-center gap-0.5 overflow-x-auto rounded-[0.625rem] bg-muted p-[3px]">
      <TabsTrigger v-for="tab in props.tabs" :key="tab.name" :value="tab.name" :class="TAB">
        {{ tab.label }}
      </TabsTrigger>
      <button
        v-if="props.more.length && phone"
        type="button"
        :class="cn(TAB, 'touch-hit')"
        :data-state="openMore ? 'active' : 'inactive'"
        :aria-current="openMore ? 'page' : undefined"
        data-route-tabs-more
        data-tour="route-tabs-more"
        @click="sheetOpen = true"
      >
        {{ openMore?.label ?? props.moreLabel }}
        <ChevronDown class="size-3.5" aria-hidden="true" />
      </button>
      <DropdownMenu v-else-if="props.more.length" align="start">
        <template #trigger>
          <button
            type="button"
            :class="cn(TAB, 'touch-hit')"
            :data-state="openMore ? 'active' : 'inactive'"
            :aria-current="openMore ? 'page' : undefined"
            data-route-tabs-more
            data-tour="route-tabs-more"
          >
            {{ openMore?.label ?? props.moreLabel }}
            <ChevronDown class="size-3.5" aria-hidden="true" />
          </button>
        </template>
        <DropdownMenuItem
          v-for="tab in props.more"
          :key="tab.name"
          :class="cn(MENU_ITEM_CLASS, 'min-w-60 flex-col items-start gap-0', tab.name === route.name && 'bg-accent')"
          @select="open(tab.name)"
        >
          <span class="font-medium">{{ tab.label }}</span>
          <span v-if="tab.hint" class="text-xs text-muted-foreground">{{ tab.hint }}</span>
        </DropdownMenuItem>
      </DropdownMenu>
    </TabsList>
  </TabsRoot>
  <ActionSheet v-if="phone && props.more.length" v-model:open="sheetOpen" :actions="moreActions" :title="props.moreTitle || props.moreLabel" />
</template>
