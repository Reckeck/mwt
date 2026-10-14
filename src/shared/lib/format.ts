import { activeLocale } from '@/shared/lib/locale'

/** ISO 4217 codes the runtime knows; anything else (BTC, USDT, …) is shown by its ticker. */
const ISO_CURRENCIES: Set<string> = (() => {
  try {
    return new Set(Intl.supportedValuesOf('currency').map((code) => code.toLowerCase()))
  } catch {
    return new Set(['usd', 'eur', 'rub', 'gbp', 'cny', 'jpy', 'chf', 'kzt', 'try', 'aed'])
  }
})()

export function isFiatCode(code: string | null | undefined): boolean {
  return !!code && ISO_CURRENCIES.has(code.toLowerCase())
}

/** Locale symbol for fiat (`₽`, `$`, `CA$`, `€`), the uppercase ticker otherwise. */
export function currencySymbol(code: string): string {
  if (!isFiatCode(code)) return code.toUpperCase()
  try {
    const part = new Intl.NumberFormat(activeLocale.value, {
      style: 'currency',
      currency: code.toUpperCase(),
      // Not `narrowSymbol`: that turns CAD, AUD and USD all into a bare `$`.
      currencyDisplay: 'symbol',
    })
      .formatToParts(0)
      .find((entry) => entry.type === 'currency')
    return part?.value ?? code.toUpperCase()
  } catch {
    return code.toUpperCase()
  }
}

export interface MoneyFormatOptions {
  /** Fixed fraction digits; by default 2 for fiat and up to 8 significant places for crypto. */
  decimals?: number
  currency?: string
  /** Prefix positive values with `+`. */
  signed?: boolean
  /** `1.2M` style for tight spots. */
  compact?: boolean
}

/** The formatted number and the currency mark, kept apart so the UI can style them. */
export function moneyParts(
  value: string | number | null | undefined,
  opts: MoneyFormatOptions = {},
): { number: string; symbol: string } {
  if (value === null || value === undefined || value === '') return { number: '—', symbol: '' }
  const { currency, signed = false, compact = false } = opts
  const numeric = typeof value === 'string' ? Number(value) : value
  if (!Number.isFinite(numeric)) return { number: String(value), symbol: currency ? currencySymbol(currency) : '' }
  const crypto = !!currency && !isFiatCode(currency)
  const decimals = opts.decimals ?? (crypto ? undefined : 2)
  const fmt = new Intl.NumberFormat(activeLocale.value, {
    notation: compact ? 'compact' : 'standard',
    signDisplay: signed ? 'exceptZero' : 'auto',
    ...(decimals === undefined
      ? { maximumFractionDigits: Math.abs(numeric) >= 1 ? 4 : 8, minimumFractionDigits: 0 }
      : { minimumFractionDigits: compact ? 0 : decimals, maximumFractionDigits: decimals }),
  })
  return { number: fmt.format(numeric), symbol: currency ? currencySymbol(currency) : '' }
}

/**
 * Money always arrives as a decimal `string` from Rust (`Amount`).
 *
 * Digits are grouped by the UI locale and the currency mark is separated by a
 * no-break space, so `42050.5USD` reads as `42 050,50 $`.
 */
export function formatMoney(value: string | number | null | undefined, opts: MoneyFormatOptions = {}): string {
  const { number, symbol } = moneyParts(value, opts)
  return symbol ? `${number}\u00a0${symbol}` : number
}

export interface PctFormatOptions {
  /** Prefix positive values with `+`. */
  signed?: boolean
  decimals?: number
}

/** `12.5` → `12,50 %` in the UI locale; the value is already in percent, not a fraction. */
export function formatPct(value: string | number | null | undefined, opts: PctFormatOptions = {}): string {
  if (value === null || value === undefined || value === '') return '—'
  const numeric = typeof value === 'string' ? Number(value) : value
  if (!Number.isFinite(numeric)) return String(value)
  const { signed = false, decimals = 2 } = opts
  return new Intl.NumberFormat(activeLocale.value, {
    style: 'percent',
    signDisplay: signed ? 'exceptZero' : 'auto',
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  }).format(numeric / 100)
}

/**
 * A holding's quantity or a count: as many places as it has, up to
 * `maxDecimals` (the user's «знаков после запятой»), never padded with zeros
 * (`1`, `0,00042`).
 */
export function formatQuantity(value: string | number | null | undefined, maxDecimals = 8): string {
  if (value === null || value === undefined || value === '') return '—'
  const numeric = typeof value === 'string' ? Number(value) : value
  if (!Number.isFinite(numeric)) return String(value)
  return new Intl.NumberFormat(activeLocale.value, { maximumFractionDigits: maxDecimals }).format(numeric)
}

/**
 * A unit price: two places from 1 up, more below it so a token at 0.00031 does
 * not read as 0.00.
 */
export function formatPrice(value: string | number | null | undefined, currency?: string): string {
  if (value === null || value === undefined || value === '') return '—'
  const numeric = typeof value === 'string' ? Number(value) : value
  if (!Number.isFinite(numeric)) return String(value)
  const abs = Math.abs(numeric)
  const decimals = abs >= 1 || abs === 0 ? 2 : Math.min(8, 2 - Math.floor(Math.log10(abs)) + 1)
  return formatMoney(numeric, { currency, decimals })
}

export type SignTone = 'positive' | 'negative' | 'neutral'

/** Which way a signed figure went; zero and empty stay neutral, never "profit". */
export function signTone(value: string | number | null | undefined): SignTone {
  if (value === null || value === undefined || value === '') return 'neutral'
  const numeric = typeof value === 'string' ? Number(value) : value
  if (!Number.isFinite(numeric) || numeric === 0) return 'neutral'
  return numeric > 0 ? 'positive' : 'negative'
}

/** Text colour for a tone: tokens, so light and dark themes need nothing extra. */
export const SIGN_TONE_CLASS: Record<SignTone, string> = {
  positive: 'text-success',
  negative: 'text-destructive',
  neutral: '',
}

/** Colour of a signed figure (P&L, a change, an amount in or out). */
export function signClass(value: string | number | null | undefined): string {
  return SIGN_TONE_CLASS[signTone(value)]
}

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return new Intl.DateTimeFormat(activeLocale.value, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
  }).format(date)
}

export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return new Intl.DateTimeFormat(activeLocale.value, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

/** Time only when the moment is today, date and time otherwise. */
export function formatAsOf(iso: string | null | undefined): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  if (date.toDateString() !== new Date().toDateString()) return formatDateTime(iso)
  return new Intl.DateTimeFormat(activeLocale.value, { hour: '2-digit', minute: '2-digit' }).format(date)
}

export function useMoney() {
  return {
    money: (value: string | number | null | undefined, opts?: MoneyFormatOptions) => formatMoney(value, opts),
    formatPct,
    quantity: formatQuantity,
    price: formatPrice,
    date: formatDate,
    dateTime: formatDateTime,
    locale: activeLocale,
  }
}
