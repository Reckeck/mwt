import type { RouteLocationRaw } from 'vue-router'

export interface GlanceProps {
  /** What the figure is (`Spent in October`): the sheet's title on a phone, the trigger's name for a reader. */
  title: string
  /** The screen behind the figure: `Open in Spending →`, and Enter on a computer. */
  openTo?: RouteLocationRaw
  /** The link's words, without the arrow. */
  openLabel?: string
  /** Classes of the trigger (a size, a weight); the dashed underline is always there. */
  triggerClass?: string
  /** Which glance, as `data-glance-trigger` names it for tests and tours. */
  kind?: string
  /** The thing it shows as `kind:id` (`entityKey`): `data-entity` of the trigger, for tests and link checks. */
  entity?: string
  /** Name of the layer for `data-overlay` (docs/ui-overlays.md). */
  overlay?: string
}
