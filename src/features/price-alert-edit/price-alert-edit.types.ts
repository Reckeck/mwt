export interface PriceAlertPreset {
  /** Ticker, e.g. `BTC`; resolved to a known asset when the dialog opens. */
  symbol: string
  /** Price in USD. */
  price: string
  condition?: AlertCondition
}

export interface PriceAlertDialogProps {
  /** An alert to edit; `null` creates a new one. */
  alert?: PriceAlert | null
  /** The exchange whose terminal sets a new alert. */
  storeId?: string | null
  /** Pre-filled asset and price for a new alert (from a chart). */
  preset?: PriceAlertPreset | null
}
