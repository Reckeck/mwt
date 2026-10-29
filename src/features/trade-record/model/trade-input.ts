import { parseDecimal } from '@/shared/lib/decimal'

export type TradeSide = 'buy' | 'sell'

/** A buy worked out elsewhere (the deal calculator) that the dialog opens filled with. */
export interface TradeDraft {
  asset: AssetRef | null
  qty: string
  price: string
}

/** The coin the price is in; an exchange spot pair is quoted in it. */
export const TRADE_QUOTE: AssetIdent = {
  symbol: 'USDT',
  name: 'Tether',
  coingecko_id: 'tether',
  chain: null,
  contract: null,
}

/** `left × right` in decimal strings, exact (no float drift like 0.05 × 90000 = 4500.000…1). */
export function decimalMul(left: string, right: string): string | null {
  const [first, second] = [parseDecimal(left), parseDecimal(right)]
  if (first === null || second === null) return null
  const scale = (value: string) => (value.split('.')[1] ?? '').length
  const digits = (value: string) => BigInt(value.replace('.', ''))
  const places = scale(first) + scale(second)
  const product = digits(first) * digits(second)
  const sign = product < 0n ? '-' : ''
  const text = (product < 0n ? -product : product).toString().padStart(places + 1, '0')
  const whole = text.slice(0, text.length - places)
  const fraction = text.slice(text.length - places).replace(/0+$/, '')
  return `${sign}${whole}${fraction ? `.${fraction}` : ''}`
}

/** `Bought 0.05 BTC at 90,000 USDT` as the spot trade the wallet's `Spot` form writes. */
export function tradeInput(
  storeId: string,
  asset: AssetRef,
  side: TradeSide,
  qty: string,
  price: string,
): TransactionInput | null {
  const amount = parseDecimal(qty)
  const total = decimalMul(qty, price)
  if (amount === null || total === null) return null
  const coin = { asset: { ...asset }, qty: amount }
  const quote = { asset: TRADE_QUOTE, qty: total }
  return {
    type: 'trade_spot',
    common: {
      timestamp: null,
      comment: '',
      categories: [],
      flow_id: null,
      contact_id: null,
      price_usd: null,
      fx_override: null,
      price: null,
      price_currency: null,
    },
    store_id: storeId,
    sold: side === 'buy' ? quote : coin,
    bought: side === 'buy' ? coin : quote,
    fee: null,
    liquidity: null,
    order_id: null,
  }
}
