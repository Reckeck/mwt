import { computed, ref, watch, watchEffect } from 'vue'
import { useI18n } from 'vue-i18n'
import { useDebounceFn } from '@vueuse/core'
import type { Layout } from 'grid-layout-plus'

import {
  DASHBOARD_COLS,
  DEFAULT_DASHBOARD,
  type DashboardWidgetDef,
  type DashboardWidgetExtras,
  hasMergedIds,
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
    const saved = repaired(settingsStore.vault?.dashboard_widgets ?? [])
    return saved.length ? saved : packWidgets(DEFAULT_DASHBOARD)
  })
  const storedSignature = computed(() => layoutSignature(stored.value))

  /**
   * Trading widgets while crypto is off: kept in the vault untouched, left off
   * the grid, and written back with every save so turning crypto on restores them.
   */
  const hiddenCfgs = computed(() =>
    settingsStore.cryptoEnabled ? [] : stored.value.filter((cfg) => widgetDef(cfg.id)?.requires === 'exchange'),
  )
  const shownCfgs = computed(() => stored.value.filter((cfg) => !hiddenCfgs.value.includes(cfg)))

  const pinnedIds = computed(() => new Set(stored.value.filter((cfg) => cfg.pinned).map((cfg) => cfg.id)))
  /** Pins as the grid currently draws them (static items). */
  const shownPinned = () => new Set(layout.value.filter((item) => item.static).map((item) => String(item.i)))
  /** What each widget keeps besides its cell: the wallet it is bound to and its own period. */
  const extras = computed(
    () =>
      new Map<string, DashboardWidgetExtras>(
        stored.value.map((cfg) => [
          cfg.id,
          { store_id: cfg.store_id ?? null, period: cfg.period ?? null, scenario_id: cfg.scenario_id ?? null },
        ]),
      ),
  )
  const cfgsOf = (items: Layout, pinned = pinnedIds.value, kept = extras.value) => [
    ...toCfgs(items, pinned, kept),
    ...hiddenCfgs.value,
  ]

  watch(
    [storedSignature, hiddenCfgs],
    ([next]) => {
      // Every vault write hands back a fresh settings object; rebuilding the grid
      // from it re-fires `layout-updated`, which used to save again — a loop.
      // The grid is only rebuilt when the vault says something it does not show yet.
      if (next === layoutSignature(cfgsOf(layout.value, shownPinned()))) return
      layout.value = toLayout(shownCfgs.value)
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

  /** A layout saved before widgets were merged is written back once under the new ids. */
  let migrated = false
  watchEffect(() => {
    const raw = settingsStore.vault?.dashboard_widgets
    if (migrated || !raw || !hasMergedIds(raw)) return
    migrated = true
    // `save` skips a layout equal to the stored one, and the renamed layout is exactly that.
    settingsStore
      .updateVault({ dashboard_widgets: stored.value })
      .catch((err) => notify.error(err, 'settings.saveFailed'))
  })

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

  /** Taking a widget off is one click; the toast's `Undo` puts the old layout back. */
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
   * On the stacked (phone) dashboard nothing can be dragged: `Move up`/`Move down` swap a widget
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
    const kept = new Map(extras.value).set(def.id, { store_id: storeId, period: null, scenario_id: null })
    void save(cfgsOf(items, pinnedIds.value, kept))
    notify.success('widgetFrame.added', { description: t(def.titleKey) })
  }

  /** The dashboard's period, unless the widget keeps its own. */
  const dashboardPeriod = computed<DashPeriod>(() => settingsStore.vault?.dashboard_period ?? 'month')
  const ownPeriod = (id: string) => extras.value.get(id)?.period ?? null

  /** `null` puts the widget back on the dashboard's period. */
  function setPeriod(id: string, period: DashPeriod | null) {
    setExtras(id, { period })
  }

  /** The scenario a `Scenario` widget follows; `null` = the newest one. */
  const ownScenario = (id: string) => extras.value.get(id)?.scenario_id ?? null
  function setScenario(id: string, scenarioId: string | null) {
    setExtras(id, { scenario_id: scenarioId })
  }

  function setExtras(id: string, patch: Partial<DashboardWidgetExtras>) {
    const kept = new Map(extras.value)
    const current = kept.get(id) ?? { store_id: null, period: null, scenario_id: null }
    kept.set(id, { ...current, ...patch })
    void save(cfgsOf(layout.value, pinnedIds.value, kept))
  }

  /** `Reset layout` (plan H6): the default set; the toast's `Undo` brings the old one back. */
  function resetLayout() {
    const before = cfgsOf(layout.value)
    void save(packWidgets(DEFAULT_DASHBOARD))
    notify.success('dashboard.resetDone', {
      action: { label: t('actions.undo'), onClick: () => void save(before) },
    })
  }

  /**
   * A widget bound to a wallet (a trading widget to its exchange) gets it as `storeId`;
   * one that follows a period gets it as `period`, a scenario widget its `scenarioId`.
   */
  function widgetProps(id: string): Record<string, unknown> {
    const def = widgetDef(id)
    const storeId = extras.value.get(id)?.store_id
    return {
      ...def?.props,
      ...(storeId ? { storeId } : {}),
      ...(def?.scenario ? { scenarioId: ownScenario(id) } : {}),
      ...(def?.periodic ? { period: ownPeriod(id) ?? dashboardPeriod.value } : {}),
    }
  }

  return {
    hydrated,
    layout,
    pinnedIds,
    dashboardPeriod,
    ownPeriod,
    saveLayout,
    setPinned,
    setPeriod,
    ownScenario,
    setScenario,
    resetLayout,
    remove,
    resize,
    move,
    add,
    widgetProps,
  }
}
