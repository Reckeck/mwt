export interface StorePositionsProps {
  positions: StorePosition[]
  baseCurrency: string
  decimals: number
  /** Card title; defaults to «Positions». */
  title?: string
  /** Rows open the asset: click or Enter emits `open`. */
  clickable?: boolean
  /** The oldest quote behind the prices, shown in the header. */
  pricesAsOf?: string | null
}
