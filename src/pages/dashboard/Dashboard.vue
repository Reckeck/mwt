<script setup lang="ts">
import { computed, ref, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useDebounceFn, useElementSize } from '@vueuse/core'
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
import { Skeleton } from '@/shared/ui/skeleton'
import type { UiAction } from '@/shared/ui/context-menu'
import { WIDGET_DRAG_HANDLE, WidgetFrame } from '@/shared/ui/widget-frame'
import { parseError } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const settingsStore = useSettingsStore()

/**
 * Below this container width the widgets are stacked one per row instead of
 * gridded. The grid's own responsive mode is not used: it derives each
 * breakpoint's layout from the previous one, so a window narrowed and
 * widened again came back with every widget squeezed into one column.
 */
const STACK_BELOW_PX = 768

/** Nothing is drawn until the vault settings land, so the defaults never flash first. */
const hydrated = computed(() => settingsStore.vault !== null)

/** The grid's own copy; it mutates it while dragging, the vault gets the result. */
const layout = ref<Layout>([])

/**
 * A widget narrower than its minimum can only come from a narrow-window
 * layout saved over the real one (every widget squeezed into one column).
 * Such a layout is packed afresh in its reading order; pins and store
 * bindings are kept.
 */
function repaired(saved: DashboardWidgetCfg[]): DashboardWidgetCfg[] {
  const damaged = saved.some((cfg) => cfg.width < (widgetDef(cfg.id)?.minW ?? 1))
  if (!damaged) return saved
  const order = [...saved].sort((left, right) => left.row - right.row || left.col - right.col)
  const byId = new Map(saved.map((cfg) => [cfg.id, cfg]))
  return packWidgets(order.map((cfg) => cfg.id)).map((cfg) => ({
    ...cfg,
    pinned: byId.get(cfg.id)?.pinned ?? false,
    store_id: byId.get(cfg.id)?.store_id ?? null,
  }))
}

/** What is on the dashboard, as stored; a fresh vault gets the default set. */
const stored = computed<DashboardWidgetCfg[]>(() => {
  const saved = (settingsStore.vault?.dashboard_widgets ?? []).filter((cfg) => widgetDef(cfg.id))
  return saved.length ? repaired(saved) : packWidgets(DEFAULT_DASHBOARD)
})

/** Grid items of the full-width layout. */
function toLayout(cfgs: DashboardWidgetCfg[]): Layout {
  return cfgs.map((cfg) => {
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
}

/** The grid mounts once its container has a real width, so it never lays out at zero. */
const container = useTemplateRef<HTMLElement>('container')
const { width: containerWidth } = useElementSize(container)
const stacked = computed(() => containerWidth.value > 0 && containerWidth.value < STACK_BELOW_PX)
/** The stack keeps the grid's reading order: top to bottom, then left to right. */
const stackOrder = computed(() => [...layout.value].sort((left, right) => left.y - right.y || left.x - right.x))

/** Order-independent fingerprint: a save that changes nothing is skipped, and so is the re-layout it would cause. */
function signature(cfgs: DashboardWidgetCfg[]): string {
  return JSON.stringify(
    cfgs
      .map((cfg) => [cfg.id, cfg.col, cfg.row, cfg.width, cfg.height, cfg.pinned, cfg.store_id ?? null])
      .sort((left, right) => String(left[0]).localeCompare(String(right[0]))),
  )
}
const storedSignature = computed(() => signature(stored.value))

const pinnedIds = computed(() => new Set(stored.value.filter((cfg) => cfg.pinned).map((cfg) => cfg.id)))
/** Pins as the grid currently draws them (static items). */
function shownPinned(): Set<string> {
  return new Set(layout.value.filter((item) => item.static).map((item) => String(item.i)))
}
const storeIds = computed(() => new Map(stored.value.map((cfg) => [cfg.id, cfg.store_id ?? null])))

function toCfgs(items: Layout, pinned: Set<string>): DashboardWidgetCfg[] {
  return items.map((item) => ({
    id: String(item.i),
    col: item.x,
    row: item.y,
    width: item.w,
    height: item.h,
    pinned: pinned.has(String(item.i)),
    store_id: storeIds.value.get(String(item.i)) ?? null,
  }))
}

watch(
  storedSignature,
  (next) => {
    // Every vault write hands back a fresh settings object; rebuilding the grid
    // from it re-fires `layout-updated`, which used to save again — a loop.
    // The grid is only rebuilt when the vault says something it does not show yet.
    if (next === signature(toCfgs(layout.value, shownPinned()))) return
    layout.value = toLayout(stored.value)
  },
  { immediate: true },
)

async function save(cfgs: DashboardWidgetCfg[]) {
  if (!hydrated.value || signature(cfgs) === storedSignature.value) return
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

/** Built once per layout/pin change instead of on every render of every frame. */
const actionsById = computed(() => new Map(layout.value.map((item) => [String(item.i), widgetActions(String(item.i))])))

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
  <div ref="container" class="relative min-h-full pb-20" data-tour="dashboard-grid">
    <div v-if="!hydrated || !containerWidth" class="grid grid-cols-1 gap-4 md:grid-cols-12">
      <Skeleton class="h-40 md:col-span-6" />
      <Skeleton class="h-40 md:col-span-6" />
      <Skeleton class="h-56 md:col-span-12" />
    </div>
    <!-- Narrow: a plain stack in reading order; the 12-column layout stays untouched. -->
    <div v-else-if="stacked" class="grid gap-3">
      <div v-for="item in stackOrder" :key="item.i" :style="{ minHeight: `${item.h * DASHBOARD_ROW_HEIGHT}px` }" class="grid">
        <WidgetFrame
          :title="t(widgetDef(String(item.i))?.titleKey ?? '')"
          :pinned="pinnedIds.has(String(item.i))"
          :actions="actionsById.get(String(item.i)) ?? []"
          @pin="(value) => setPinned(String(item.i), value)"
          @remove="remove(String(item.i))"
        >
          <component :is="widgetDef(String(item.i))?.component" v-bind="widgetDef(String(item.i))?.props ?? {}" />
        </WidgetFrame>
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
          :actions="actionsById.get(String(item.i)) ?? []"
          @pin="(value) => setPinned(String(item.i), value)"
          @remove="remove(String(item.i))"
        >
          <component :is="widgetDef(String(item.i))?.component" v-bind="widgetDef(String(item.i))?.props ?? {}" />
        </WidgetFrame>
      </GridItem>
    </GridLayout>

    <div v-if="hydrated && !layout.length" class="py-16 text-center text-sm text-muted-foreground">
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
