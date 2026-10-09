/**
 * Upper bound on rendered options.
 *
 * The catalog is the whole of CoinGecko `/coins/list` — thousands of tickers —
 * so an unfiltered list would build thousands of DOM nodes inside an already
 * open panel. Typing narrows the list long before this bites; when matches are
 * truncated, the panel says how many were hidden instead of silently lying.
 */
export const MAX_CURRENCY_OPTIONS = 300

export interface CurrencySelectProps {
  /** Label above the control; pass `''` to hide. */
  label?: string
  placeholder?: string
  disabled?: boolean
}
