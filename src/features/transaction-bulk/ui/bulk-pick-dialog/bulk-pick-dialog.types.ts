export type BulkPickKind = 'category' | 'store'

export interface BulkPickDialogProps {
  kind: BulkPickKind
  /** How many operations change: «Категория для 2 операций». */
  count: number
  pending?: boolean
}
