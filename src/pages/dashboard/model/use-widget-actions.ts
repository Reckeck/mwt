import { type ComputedRef, type Ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { Layout } from 'grid-layout-plus'
import { LayoutGrid, Lock, Maximize2, PinOff, X } from 'lucide-vue-next'

import type { UiAction } from '@/shared/ui/context-menu'

interface WidgetActionDeps {
  layout: Ref<Layout>
  pinnedIds: ComputedRef<Set<string>>
  hasMore: ComputedRef<boolean>
  setPinned: (id: string, pinned: boolean) => void
  resize: (id: string, size: 's' | 'm' | 'l') => void
  remove: (id: string) => void
  openCatalog: () => void
}

/** The menu of each widget — the same list behind right click and «⋯». */
export function useWidgetActions(deps: WidgetActionDeps) {
  const { t } = useI18n()

  function widgetActions(id: string): UiAction[] {
    const pinned = deps.pinnedIds.value.has(id)
    return [
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
  return computed(() => new Map(deps.layout.value.map((item) => [String(item.i), widgetActions(String(item.i))])))
}
