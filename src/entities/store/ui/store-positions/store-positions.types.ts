export interface StorePositionsProps {
  positions: StorePosition[]
  baseCurrency: string
  decimals: number
  /** Card title; defaults to «Positions». */
  title?: string
}
