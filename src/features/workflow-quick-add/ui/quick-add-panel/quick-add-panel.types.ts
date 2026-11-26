import type { QuickAddContext } from '../../model/quick-add'

export interface QuickAddPanelProps {
  /** Screen position of the panel's top-left corner. */
  left: number
  top: number
  context: QuickAddContext
  /** In a sheet (a phone): fills it instead of floating at `left`/`top`. */
  docked?: boolean
  /** Hide nodes marked advanced. */
  simple: boolean
}
