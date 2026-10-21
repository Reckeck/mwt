<script setup lang="ts">
import { computed, ref, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useElementSize } from '@vueuse/core'
import { GridItem, GridLayout } from 'grid-layout-plus'
import { LayoutGrid } from 'lucide-vue-next'

import { type DashboardCatalogExchange, DashboardCatalog } from '@/widgets/dashboard-catalog'
import { DASHBOARD_COLS, DASHBOARD_ROW_HEIGHT, DASHBOARD_WIDGETS } from '@/widgets/dashboard-registry'
import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'
import { Button } from '@/shared/ui/button'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { Skeleton } from '@/shared/ui/skeleton'
import { WIDGET_DRAG_HANDLE } from '@/shared/ui/widget-frame'

import { useDashboardLayout } from './model/use-dashboard-layout'
import { useWidgetActions } from './model/use-widget-actions'
import { DashboardWidgetSlot } from './ui/dashboard-widget-slot'
import { useStoreTree } from '@/shared/api/queries'

const { t } = useI18n()

/**
 * Below this container width the widgets are stacked one per row instead of
 * gridded. The grid's own responsive mode is not used: it derives each
 * breakpoint's layout from the previous one, so a window narrowed and
 * widened again came back with every widget squeezed into one column.
 */
const STACK_BELOW_PX = 768
/** Half the band around the switch: a window resized across 768 px does not flip every pixel. */
const STACK_HYSTERESIS_PX = 16

const {
  hydrated,
  layout,
  pinnedIds,
  dashboardPeriod,
  ownPeriod,
  saveLayout,
  setPinned,
  setPeriod,
  remove,
  resize,
  move,
  add,
  widgetProps,
} = useDashboardLayout()

/** One period for every widget that follows it; a widget may keep its own (its menu). */
const PERIODS: DashPeriod[] = ['month', 'quarter', 'year']
const settingsStore = useSettingsStore()
function choosePeriod(value: unknown) {
  // A single toggle group hands back `undefined` when the pressed item is clicked again.
  if (typeof value !== 'string' || value === dashboardPeriod.value) return
  settingsStore
    .updateVault({ dashboard_period: value as DashPeriod })
    .catch((err) => notify.error(err, 'settings.saveFailed'))
}

/** The grid mounts once its container has a real width, so it never lays out at zero. */
const container = useTemplateRef<HTMLElement>('container')
const { width: containerWidth } = useElementSize(container)
const stacked = ref(false)
/** The first width decides inside the band; after that only leaving it flips the layout. */
let settledOnce = false
watch(
  containerWidth,
  (width) => {
    if (!width) return
    if (width < STACK_BELOW_PX - STACK_HYSTERESIS_PX) stacked.value = true
    else if (width >= STACK_BELOW_PX + STACK_HYSTERESIS_PX) stacked.value = false
    else if (!settledOnce) stacked.value = width < STACK_BELOW_PX
    settledOnce = true
  },
  { immediate: true },
)

/**
 * The grid animates every move of an item; on the first layout that is every
 * widget sliding in from the corner. Transitions start once it has settled.
 */
const animate = ref(false)
function onLayoutReady() {
  requestAnimationFrame(() => requestAnimationFrame(() => (animate.value = true)))
}
/** The stack keeps the grid's reading order: top to bottom, then left to right. */
const stackOrder = computed(() => [...layout.value].sort((left, right) => left.y - right.y || left.x - right.x))

/* --- catalog: widgets not yet on the dashboard --- */
const catalogOpen = ref(false)
const placed = computed(() => layout.value.map((item) => String(item.i)))

/** Trading widgets are offered only while the vault has an exchange with a provider. */
const tree = useStoreTree()
const exchanges = computed<DashboardCatalogExchange[]>(() =>
  (tree.data.value ?? [])
    .map((node) => node.store)
    .filter((store) => store.kind === 'exchange' && !store.archived && store.exchange)
    .map((store) => ({ id: store.id, name: store.name })),
)
const hasMore = computed(() =>
  DASHBOARD_WIDGETS.some((def) => !placed.value.includes(def.id) && (!def.requires || exchanges.value.length)),
)

const actionsById = useWidgetActions({
  layout,
  pinnedIds,
  hasMore,
  stacked,
  move,
  setPinned,
  resize,
  remove,
  ownPeriod,
  setPeriod,
  openCatalog: () => {
    catalogOpen.value = true
  },
})
</script>

<template>
  <div ref="container" class="relative min-h-full pb-6" data-tour="dashboard-grid">
    <!-- The catalog of widgets not yet on the dashboard; the transaction button lives in the header. -->
    <!-- The row is always there: the button appearing once the wallets load must not push the grid down. -->
    <div class="mb-3 flex items-center justify-between gap-2">
      <ToggleGroup
        :model-value="dashboardPeriod"
        type="single"
        variant="outline"
        size="sm"
        :aria-label="t('dashboard.period.label')"
        data-tour="dashboard-period"
        @update:model-value="choosePeriod"
      >
        <ToggleGroupItem v-for="period in PERIODS" :key="period" :value="period" class="px-2.5 text-xs">
          {{ t(`dashboard.period.${period}`) }}
        </ToggleGroupItem>
      </ToggleGroup>
      <Button
        variant="outline"
        size="sm"
        data-tour="dashboard-add"
        :class="!hasMore && 'invisible'"
        :aria-hidden="!hasMore"
        :tabindex="hasMore ? undefined : -1"
        @click="catalogOpen = true"
      >
        <LayoutGrid class="size-4" />
        {{ t('dashboard.catalog.button') }}
      </Button>
    </div>
    <div v-if="!hydrated || !containerWidth" class="grid grid-cols-1 gap-4 md:grid-cols-12">
      <Skeleton class="h-40 md:col-span-6" />
      <Skeleton class="h-40 md:col-span-6" />
      <Skeleton class="h-56 md:col-span-12" />
    </div>
    <!-- Narrow: a plain stack in reading order; the 12-column layout stays untouched. -->
    <div v-else-if="stacked" class="stack grid gap-3">
      <div v-for="item in stackOrder" :key="item.i" :style="{ minHeight: `${item.h * DASHBOARD_ROW_HEIGHT}px` }" class="grid">
        <DashboardWidgetSlot
          :id="String(item.i)"
          :pinned="pinnedIds.has(String(item.i))"
          :actions="actionsById.get(String(item.i)) ?? []"
          :widget-props="widgetProps(String(item.i))"
          @pin="(value) => setPinned(String(item.i), value)"
        />
      </div>
    </div>
    <GridLayout
      v-else
      v-model:layout="layout"
      :col-num="DASHBOARD_COLS"
      :row-height="DASHBOARD_ROW_HEIGHT"
      :margin="[12, 12]"
      :vertical-compact="true"
      :use-css-transforms="true"
      :class="!animate && 'no-motion'"
      @layout-ready="onLayoutReady"
      @layout-updated="saveLayout"
    >
      <GridItem
        v-for="item in layout"
        :key="item.i"
        :i="item.i"
        :x="item.x"
        :y="item.y"
        :w="item.w"
        :h="item.h"
        :min-w="item.minW"
        :min-h="item.minH"
        :static="item.static"
        :is-resizable="item.isResizable"
        :drag-allow-from="`.${WIDGET_DRAG_HANDLE}`"
      >
        <DashboardWidgetSlot
          :id="String(item.i)"
          :pinned="pinnedIds.has(String(item.i))"
          :actions="actionsById.get(String(item.i)) ?? []"
          :widget-props="widgetProps(String(item.i))"
          @pin="(value) => setPinned(String(item.i), value)"
        />
      </GridItem>
    </GridLayout>

    <div v-if="hydrated && !layout.length" class="py-16 text-center text-sm text-muted-foreground">
      {{ t('dashboard.emptyGrid') }}
    </div>

    <DashboardCatalog v-model:open="catalogOpen" :placed="placed" :exchanges="exchanges" @add="add" />
  </div>
</template>

<style scoped>
/* The grid's placeholder and resize corner in theme colours. */
:deep(.vgl-layout) {
  --vgl-placeholder-bg: var(--primary);
  --vgl-placeholder-opacity: 15%;
  --vgl-resizer-border-color: var(--muted-foreground);
}
/* The stack moves by «Выше»/«Ниже» in the menu: its headers are not drag handles. */
.stack :deep(.widget-drag-handle) {
  cursor: auto;
}
.no-motion,
.no-motion :deep(.vgl-item) {
  transition: none;
}
:deep(.vgl-item--placeholder) {
  border-radius: 0.75rem;
}
</style>
