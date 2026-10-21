import { type InjectionKey, type Ref, computed, inject, provide } from 'vue'
import { useElementSize } from '@vueuse/core'

/** How much a widget shows (plan H1): S a figure and its change, M + a trend, L + the breakdown. */
export type WidgetSize = 's' | 'm' | 'l'

/** Below either of these a cell is «S», px. */
export const WIDGET_S_BELOW = { width: 320, height: 160 } as const
/** From both of these a cell is «L», px. */
export const WIDGET_L_FROM = { width: 560, height: 300 } as const

const WIDGET_SIZE_KEY: InjectionKey<Ref<WidgetSize>> = Symbol('widget-size')

/** The level of a cell of the given size; an unmeasured cell counts as «M». */
export function widgetSizeOf(width: number, height: number): WidgetSize {
  if (!width || !height) return 'm'
  if (width < WIDGET_S_BELOW.width || height < WIDGET_S_BELOW.height) return 's'
  if (width >= WIDGET_L_FROM.width && height >= WIDGET_L_FROM.height) return 'l'
  return 'm'
}

/** The frame measures its body and hands the level down. */
export function provideWidgetSize(body: Ref<HTMLElement | null>): Ref<WidgetSize> {
  const { width, height } = useElementSize(body)
  const size = computed(() => widgetSizeOf(width.value, height.value))
  provide(WIDGET_SIZE_KEY, size)
  return size
}

/** The level of the cell the widget sits in; «M» outside a dashboard frame. */
export function useWidgetSize(): Ref<WidgetSize> {
  return inject(
    WIDGET_SIZE_KEY,
    computed(() => 'm' as const),
  )
}
