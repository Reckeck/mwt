import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useDebounceFn } from '@vueuse/core'
import type { Layout } from 'grid-layout-plus'

import {
  DASHBOARD_COLS,
  DEFAULT_DASHBOARD,
  type DashboardWidgetDef,
  layoutSignature,
  packWidgets,
  repaired,
  toCfgs,
  toLayout,
  widgetDef,
} from '@/widgets/dashboard-registry'
import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'

/**
 * The dashboard's widgets: the grid's working copy and the vault's stored
 * one, kept in step without a save loop, plus every change to them.
 */
export function useDashboardLayout() {
  const { t } = useI18n()
  const settingsStore = useSettingsStore()

  /** Nothing is drawn until the vault settings land, so the defaults never flash first. */
  const hydrated = computed(() => settingsStore.vault !== null)

  /** The grid's own copy; it mutates it while dragging, the vault gets the result. */
  const layout = ref<Layout>([])

  /** What is on the dashboard, as stored; a fresh vault gets the default set. */
  const stored = computed<DashboardWidgetCfg[]>(() => {
    const saved = (settingsStore.vault?.dashboard_widgets ?? []).filter((cfg) => widgetDef(cfg.id))
    return saved.length ? repaired(saved) : packWidgets(DEFAULT_DASHBOARD)
  })
  const storedSignature = computed(() => layoutSignature(stored.value))

  const pinnedIds = computed(() => new Set(stored.value.filter((cfg) => cfg.pinned).map((cfg) => cfg.id)))
  /** Pins as the grid currently draws them (static items). */
  const shownPinned = () => new Set(layout.value.filter((item) => item.static).map((item) => String(item.i)))
  const storeIds = computed(() => new Map(stored.value.map((cfg) => [cfg.id, cfg.store_id ?? null])))
  const cfgsOf = (items: Layout, pinned = pinnedIds.value, bound = storeIds.value) => toCfgs(items, pinned, bound)

  watch(
    storedSignature,
    (next) => {
      // Every vault write hands back a fresh settings object; rebuilding the grid
      // from it re-fires `layout-updated`, which used to save again — a loop.
      // The grid is only rebuilt when the vault says something it does not show yet.
      if (next === layoutSignature(cfgsOf(layout.value, shownPinned()))) return
      layout.value = toLayout(stored.value)
    },
    { immediate: true },
  )

  /**
   * Writes the layout without a toast: a moved, resized or pinned widget is
   * already visible, and the grid also saves itself after it settles on load.
   * Adding and removing a widget say so themselves.
   */
  async function save(cfgs: DashboardWidgetCfg[]) {
    if (!hydrated.value || layoutSignature(cfgs) === storedSignature.value) return
    try {
      await settingsStore.updateVault({ dashboard_widgets: cfgs })
    } catch (err) {
      notify.error(err, 'settings.saveFailed')
    }
  }

  /** Drags fire many updates; only the settled layout is written. */
  const saveLayout = useDebounceFn((items: Layout) => {
    void save(cfgsOf(items))
  }, 600)

  function setPinned(id: string, pinned: boolean) {
    const next = new Set(pinnedIds.value)
    if (pinned) next.add(id)
    else next.delete(id)
    void save(cfgsOf(layout.value, next))
  }

  /** Taking a widget off is one click; the toast's «Отменить» puts the old layout back. */
  function remove(id: string) {
    const before = cfgsOf(layout.value)
    void save(cfgsOf(layout.value.filter((item) => item.i !== id)))
    notify.success('widgetFrame.removed', {
      description: t(widgetDef(id)?.titleKey ?? ''),
      action: { label: t('actions.undo'), onClick: () => void save(before) },
    })
  }

  /**
   * S / M / L from the widget's own sizes: its minimum, its default, the full width at
   * its default height. One fixed set squeezed a calendar's eight rows into four and
   * gave a wide table half a row.
   */
  function resize(id: string, size: 's' | 'm' | 'l') {
    const def = widgetDef(id)
    if (!def) return
    const presets = { s: [def.minW, def.minH], m: [def.w, def.h], l: [DASHBOARD_COLS, def.h] } as const
    const [width, height] = presets[size]
    const items = layout.value.map((item) => (item.i === id ? { ...item, w: width, h: height } : item))
    void save(cfgsOf(items))
  }

  /**
   * On the stacked (phone) dashboard nothing can be dragged: «Выше»/«Ниже» swap a widget
   * with its neighbour in reading order, and the grid closes any gap.
   */
  function move(id: string, step: -1 | 1) {
    const order = [...layout.value].sort((left, right) => left.y - right.y || left.x - right.x)
    const index = order.findIndex((item) => item.i === id)
    const other = order[index + step]
    const self = order[index]
    if (!self || !other) return
    const items = layout.value.map((item) => {
      if (item.i === self.i) return { ...item, x: other.x, y: other.y }
      if (item.i === other.i) return { ...item, x: self.x, y: self.y }
      return item
    })
    void save(cfgsOf(items))
  }

  /** A new widget lands at the bottom; the grid pulls it up into any gap. */
  function add(def: DashboardWidgetDef, storeId: string | null) {
    const bottom = layout.value.reduce((max, item) => Math.max(max, item.y + item.h), 0)
    const items: Layout = [...layout.value, { i: def.id, x: 0, y: bottom, w: def.w, h: def.h }]
    void save(cfgsOf(items, pinnedIds.value, new Map([...storeIds.value, [def.id, storeId]])))
    notify.success('widgetFrame.added', { description: t(def.titleKey) })
  }

  /** A widget bound to a wallet (a trading widget to its exchange) gets it as `storeId`. */
  function widgetProps(id: string): Record<string, unknown> {
    const def = widgetDef(id)
    const storeId = storeIds.value.get(id)
    return storeId ? { ...def?.props, storeId } : (def?.props ?? {})
  }

  return { hydrated, layout, pinnedIds, saveLayout, setPinned, remove, resize, move, add, widgetProps }
}
