import type { RouteLocationRaw } from 'vue-router'

export interface GlanceProps {
  /** What the figure is («Потрачено в октябре»): the sheet's title on a phone, the trigger's name for a reader. */
  title: string
  /** The screen behind the figure: «Открыть в Тратах →», and Enter on a computer. */
  openTo?: RouteLocationRaw
  /** The link's words, without the arrow. */
  openLabel?: string
  /** Classes of the trigger (a size, a weight); the dashed underline is always there. */
  triggerClass?: string
  /** Which glance, as `data-glance-trigger` names it for tests and tours. */
  kind?: string
}
