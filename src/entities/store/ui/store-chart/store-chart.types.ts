export interface StoreChartProps {
  storeId: string
  baseCurrency: string
  /** Value the sub-accounts together with the store. Default on. */
  includeChildren?: boolean
}
