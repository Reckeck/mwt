<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useDebounceFn } from '@vueuse/core'
import { GridItem, GridLayout, type Layout } from 'grid-layout-plus'
import { EyeOff, LayoutGrid, Maximize2, PinOff, Plus, X } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { DashboardCatalog } from '@/widgets/dashboard-catalog'
import {
  DASHBOARD_COLS,
  DASHBOARD_ROW_HEIGHT,
  DASHBOARD_WIDGETS,
  DEFAULT_DASHBOARD,
  type DashboardWidgetDef,
  packWidgets,
  widgetDef,
} from '@/widgets/dashboard-registry'
import { Button } from '@/shared/ui/button'
import type { UiAction } from '@/shared/ui/context-menu'
import { WIDGET_DRAG_HANDLE, WidgetFrame } from '@/shared/ui/widget-frame'
import { parseError } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const settingsStore = useSettingsStore()

/** What is on the dashboard, as stored; a fresh vault gets the default set. */
const stored = computed<DashboardWidgetCfg[]>(() => {
  const saved = settingsStore.vault?.dashboard_widgets ?? []
  return saved.length ? saved.filter((cfg) => widgetDef(cfg.id)) : packWidgets(DEFAULT_DASHBOARD)
})

/** The grid's own copy; it mutates it while dragging, the vault gets the result. */
const layout = ref<Layout>([])
watch(
  stored,
  (cfgs) => {
    layout.value = cfgs.map((cfg) => {
      const def = widgetDef(cfg.id)
      return {
        i: cfg.id,
        x: cfg.col,
        y: cfg.row,
        w: cfg.width,
        h: cfg.height,
        minW: def?.minW,
        minH: def?.minH,
        static: cfg.pinned,
        isResizable: !cfg.pinned,
      }
    })
  },
  { immediate: true },
)

const pinnedIds = computed(() => new Set(stored.value.filter((cfg) => cfg.pinned).map((cfg) => cfg.id)))

function toCfgs(items: Layout, pinned: Set<string>): DashboardWidgetCfg[] {
  return items.map((item) => ({
    id: String(item.i),
    col: item.x,
    row: item.y,
    width: item.w,
    height: item.h,
    pinned: pinned.has(String(item.i)),
    store_id: null,
  }))
}

async function save(cfgs: DashboardWidgetCfg[]) {
  try {
    await settingsStore.updateVault({ dashboard_widgets: cfgs })
  } catch (err) {
    toast.error(t('settings.saveFailed'), { description: parseError(err).message })
  }
}

/** Drags fire many updates; only the settled layout is written. */
const saveLayout = useDebounceFn((items: Layout) => {
  void save(toCfgs(items, pinnedIds.value))
}, 600)

function setPinned(id: string, pinned: boolean) {
  const next = new Set(pinnedIds.value)
  if (pinned) next.add(id)
  else next.delete(id)
  void save(toCfgs(layout.value, next))
}

function remove(id: string) {
  void save(toCfgs(layout.value.filter((item) => item.i !== id), pinnedIds.value))
}

/** S / M / L presets, in grid columns × rows, never below the widget's minimum. */
function resize(id: string, size: 's' | 'm' | 'l') {
  const def = widgetDef(id)
  if (!def) return
  const presets = { s: [3, 4], m: [6, 6], l: [12, 8] } as const
  const [width, height] = presets[size]
  const items = layout.value.map((item) =>
    item.i === id ? { ...item, w: Math.max(width, def.minW), h: Math.max(height, def.minH) } : item,
  )
  void save(toCfgs(items, pinnedIds.value))
}

function widgetActions(id: string): UiAction[] {
  const pinned = pinnedIds.value.has(id)
  return [
    pinned
      ? { id: 'unpin', label: t('widgetFrame.unpin'), icon: PinOff, run: () => setPinned(id, false) }
      : { id: 'pin', label: t('widgetFrame.pin'), icon: EyeOff, run: () => setPinned(id, true) },
    {
      id: 'size',
      label: t('widgetFrame.size'),
      icon: Maximize2,
      hidden: pinned,
      children: (['s', 'm', 'l'] as const).map((size) => ({
        id: `size-${size}`,
        label: t(`widgetFrame.size_${size}`),
        run: () => resize(id, size),
      })),
    },
    { id: 'catalog', label: t('dashboard.catalog.open'), icon: LayoutGrid, separated: true, hidden: !hasMore.value, run: () => (catalogOpen.value = true) },
    { id: 'remove', label: t('widgetFrame.remove'), icon: X, destructive: true, separated: true, run: () => remove(id) },
  ]
}

/* --- catalog: a new widget lands at the bottom, the grid pulls it up into any gap --- */
const catalogOpen = ref(false)
const placed = computed(() => layout.value.map((item) => String(item.i)))
const hasMore = computed(() => placed.value.length < DASHBOARD_WIDGETS.length)

function add(def: DashboardWidgetDef) {
  const bottom = layout.value.reduce((max, item) => Math.max(max, item.y + item.h), 0)
  const items: Layout = [...layout.value, { i: def.id, x: 0, y: bottom, w: def.w, h: def.h }]
  void save(toCfgs(items, pinnedIds.value))
}
</script>

<template>
  <div class="relative min-h-full pb-20" data-tour="dashboard-grid">
    <GridLayout
      v-model:layout="layout"
      :col-num="DASHBOARD_COLS"
      :row-height="DASHBOARD_ROW_HEIGHT"
      :margin="[16, 16]"
      :vertical-compact="true"
      :use-css-transforms="true"
      :responsive="true"
      :cols="{ lg: 12, md: 12, sm: 6, xs: 2, xxs: 2 }"
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
        <WidgetFrame
          :title="t(widgetDef(String(item.i))?.titleKey ?? '')"
          :pinned="pinnedIds.has(String(item.i))"
          :actions="widgetActions(String(item.i))"
          @pin="(value) => setPinned(String(item.i), value)"
          @remove="remove(String(item.i))"
        >
          <component :is="widgetDef(String(item.i))?.component" v-bind="widgetDef(String(item.i))?.props ?? {}" />
        </WidgetFrame>
      </GridItem>
    </GridLayout>

    <div v-if="!layout.length" class="py-16 text-center text-sm text-muted-foreground">
      {{ t('dashboard.emptyGrid') }}
    </div>

    <!-- Floating «+»: the catalog of widgets not yet on the dashboard. -->
    <Button
      v-if="hasMore"
      size="icon"
      class="fixed bottom-6 right-6 z-40 size-12 rounded-full shadow-lg"
      :title="t('dashboard.catalog.open')"
      :aria-label="t('dashboard.catalog.open')"
      data-tour="dashboard-add"
      @click="catalogOpen = true"
    >
      <Plus class="size-5" />
    </Button>

    <DashboardCatalog v-model:open="catalogOpen" :placed="placed" @add="add" />
  </div>
</template>

<style scoped>
/* The grid's placeholder and resize corner in theme colours. */
:deep(.vgl-layout) {
  --vgl-placeholder-bg: var(--primary);
  --vgl-placeholder-opacity: 15%;
  --vgl-resizer-border-color: var(--muted-foreground);
}
:deep(.vgl-item--placeholder) {
  border-radius: 0.75rem;
}
</style>
