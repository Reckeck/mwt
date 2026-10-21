export interface ValuationListProps {
  valuations: Valuation[]
  /** The starting point: the purchase or the old hand-set price. */
  base: { kind: ValuationBaseKind; date: string; price: string; currency: string } | null
  pending?: boolean
}
