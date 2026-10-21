export interface AssetDetailsSheetProps {
  /** The position the sheet is about; `null` keeps the sheet empty while it closes. */
  position: StorePosition | null
  /** The wallet the position is on: the links narrow to it. */
  storeId: string
  baseCurrency: string
  decimals: number
  /** A property's thing: its value is set by dated valuations. */
  revaluable?: boolean
}
