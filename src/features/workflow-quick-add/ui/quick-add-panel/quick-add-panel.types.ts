import type { QuickAddContext } from '../../model/quick-add'

export interface QuickAddPanelProps {
  /** Screen position of the panel's top-left corner. */
  left: number
  top: number
  context: QuickAddContext
  /** Hide nodes marked advanced. */
  simple: boolean
}
