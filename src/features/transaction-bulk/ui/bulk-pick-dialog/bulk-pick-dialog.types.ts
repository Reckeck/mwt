export type BulkPickKind = 'category' | 'store'

export interface BulkPickDialogProps {
  kind: BulkPickKind
  /** How many operations change: `Category for 2 operations`. */
  count: number
  pending?: boolean
}
