import { type ComputedRef, type Ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { Layout } from 'grid-layout-plus'
import { ArrowDown, ArrowUp, CalendarRange, Calculator, LayoutGrid, Lock, Maximize2, PinOff, X } from 'lucide-vue-next'

import { widgetDef } from '@/widgets/dashboard-registry'

import type { UiAction } from '@/shared/ui/context-menu'

interface WidgetActionDeps {
  layout: Ref<Layout>
  pinnedIds: ComputedRef<Set<string>>
  hasMore: ComputedRef<boolean>
  /** The phone's one-column stack: moving is by menu, not by drag. */
  stacked: Ref<boolean>
  move: (id: string, step: -1 | 1) => void
  setPinned: (id: string, pinned: boolean) => void
  resize: (id: string, size: 's' | 'm' | 'l') => void
  remove: (id: string) => void
  /** The widget's own period, `null` while it follows the dashboard. */
  ownPeriod: (id: string) => DashPeriod | null
  setPeriod: (id: string, period: DashPeriod | null) => void
  /** Saved calculator scenarios, newest first, for the `Scenario` widget. */
  scenarios: ComputedRef<CalcScenario[]>
  ownScenario: (id: string) => string | null
  setScenario: (id: string, scenarioId: string | null) => void
  openCatalog: () => void
}

/** The menu of each widget — the same list behind right click and «⋯». */
export function useWidgetActions(deps: WidgetActionDeps) {
  const { t } = useI18n()

  function widgetActions(id: string): UiAction[] {
    const pinned = deps.pinnedIds.value.has(id)
    const order = [...deps.layout.value].sort((left, right) => left.y - right.y || left.x - right.x)
    const index = order.findIndex((item) => String(item.i) === id)
    const stack = deps.stacked.value
    return [
      {
        id: 'up',
        label: t('widgetFrame.moveUp'),
        icon: ArrowUp,
        hidden: !stack || pinned || index <= 0,
        run: () => deps.move(id, -1),
      },
      {
        id: 'down',
        label: t('widgetFrame.moveDown'),
        icon: ArrowDown,
        hidden: !stack || pinned || index === order.length - 1,
        run: () => deps.move(id, 1),
      },
      pinned
        ? { id: 'unpin', label: t('widgetFrame.unpin'), icon: PinOff, run: () => deps.setPinned(id, false) }
        : { id: 'pin', label: t('widgetFrame.pin'), icon: Lock, run: () => deps.setPinned(id, true) },
      {
        id: 'size',
        label: t('widgetFrame.size'),
        icon: Maximize2,
        hidden: pinned,
        children: (['s', 'm', 'l'] as const).map((size) => ({
          id: `size-${size}`,
          label: t(`widgetFrame.size_${size}`),
          run: () => deps.resize(id, size),
        })),
      },
      {
        id: 'period',
        label: t('widgetFrame.period'),
        icon: CalendarRange,
        hidden: !widgetDef(id)?.periodic,
        children: ([null, 'month', 'quarter', 'year'] as const).map((period) => ({
          id: `period-${period ?? 'dashboard'}`,
          // The menu has no checked state: the chosen option is marked in its label.
          label: `${deps.ownPeriod(id) === period ? '✓ ' : ''}${t(period ? `dashboard.period.${period}` : 'widgetFrame.periodDashboard')}`,
          run: () => deps.setPeriod(id, period),
        })),
      },
      {
        id: 'scenario',
        label: t('dashboard.scenarioPick'),
        icon: Calculator,
        hidden: !widgetDef(id)?.scenario || !deps.scenarios.value.length,
        children: [null, ...deps.scenarios.value.map((scenario) => scenario.id)].map((scenarioId) => ({
          id: `scenario-${scenarioId ?? 'latest'}`,
          label: `${deps.ownScenario(id) === scenarioId ? '✓ ' : ''}${
            scenarioId
              ? (deps.scenarios.value.find((scenario) => scenario.id === scenarioId)?.name ?? '')
              : t('dashboard.scenarioLatest')
          }`,
          run: () => deps.setScenario(id, scenarioId),
        })),
      },
      {
        id: 'catalog',
        label: t('dashboard.catalog.open'),
        icon: LayoutGrid,
        separated: true,
        hidden: !deps.hasMore.value,
        run: deps.openCatalog,
      },
      {
        id: 'remove',
        label: t('widgetFrame.remove'),
        icon: X,
        destructive: true,
        separated: true,
        run: () => deps.remove(id),
      },
    ]
  }

  /** Built once per layout/pin change instead of on every render of every frame. */
  return computed(() => {
    void deps.stacked.value
    void deps.scenarios.value
    return new Map(deps.layout.value.map((item) => [String(item.i), widgetActions(String(item.i))]))
  })
}
