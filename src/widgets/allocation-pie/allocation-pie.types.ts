export interface AllocationSlice {
  label: string
  /** Value in the base currency, a decimal string. */
  value: string
}

export interface AllocationPieProps {
  slices: AllocationSlice[]
  /** Card title; without it (and with `bare`) only the chart is drawn. */
  title?: string
  /** No card around it: for embedding inside another card. */
  bare?: boolean
  /** Base currency of the values, for the tooltip. */
  currency?: string
}
