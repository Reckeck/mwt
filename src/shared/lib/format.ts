// check-ui-ignore intl-cache: the one place that builds Intl formatters, each cached below
import { activeLocale } from '@/shared/lib/locale'

const numberFormats = new Map<string, Intl.NumberFormat>()
const dateFormats = new Map<string, Intl.DateTimeFormat>()

/**
 * A number formatter for the UI language, built once per options. Building one
 * costs ~30 µs: a page of sums built two per sum (rule 17). Reading
 * `activeLocale` keeps a template re-rendering when the language changes.
 */
export function numberFormat(
  options: Intl.NumberFormatOptions = {},
  locale: string = activeLocale.value,
): Intl.NumberFormat {
  const key = `${locale}|${JSON.stringify(options)}`
  let format = numberFormats.get(key)
  if (!format) {
    format = new Intl.NumberFormat(locale, options)
    numberFormats.set(key, format)
  }
  return format
}

/** A date formatter for the UI language, built once per options (see `numberFormat`). */
export function dateFormat(
  options: Intl.DateTimeFormatOptions = {},
  locale: string = activeLocale.value,
): Intl.DateTimeFormat {
  const key = `${locale}|${JSON.stringify(options)}`
  let format = dateFormats.get(key)
  if (!format) {
    format = new Intl.DateTimeFormat(locale, options)
    dateFormats.set(key, format)
  }
  return format
}

const symbols = new Map<string, string>()

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
  const key = `${activeLocale.value}|${code}`
  const known = symbols.get(key)
  if (known) return known
  let symbol = code.toUpperCase()
  try {
    const part = numberFormat({
      style: 'currency',
      currency: code.toUpperCase(),
      // Not `narrowSymbol`: that turns CAD, AUD and USD all into a bare `$`.
      currencyDisplay: 'symbol',
    })
      .formatToParts(0)
      .find((entry) => entry.type === 'currency')
    symbol = part?.value ?? symbol
  } catch {
    // An unknown code shows as its ticker.
  }
  symbols.set(key, symbol)
  return symbol
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
  const fmt = numberFormat({
    notation: compact ? 'compact' : 'standard',
    // 'negative', not 'auto': -0 and -0.001 read «0,00», not «−0,00».
    signDisplay: signed ? 'exceptZero' : 'negative',
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
  return numberFormat({
    style: 'percent',
    // 'negative', not 'auto': -0 and -0.001 read «0,00», not «−0,00».
    signDisplay: signed ? 'exceptZero' : 'negative',
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
  return numberFormat({ maximumFractionDigits: maxDecimals }).format(numeric)
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

/** «пн, 14 окт.»: a day header in a journal, where the weekday matters more than the year. */
export function formatDay(iso: string | null | undefined): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return dateFormat({ weekday: 'short', day: 'numeric', month: 'short' }).format(date)
}

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return dateFormat({ year: 'numeric', month: 'short', day: '2-digit' }).format(date)
}

/**
 * A month by its `YYYY-MM` key (`2026-10` → «октябрь 2026 г.» / «окт.»). Noon
 * of the 1st, so no time zone moves it into the month before.
 */
export function formatMonthKey(
  key: string,
  options: { month?: 'long' | 'short' | 'narrow'; year?: 'numeric' | '2-digit' } = { month: 'long', year: 'numeric' },
  locale: string = activeLocale.value,
): string {
  const date = new Date(`${key.slice(0, 7)}-01T12:00`)
  if (Number.isNaN(date.getTime())) return key
  return dateFormat(options, locale).format(date)
}

/** A weekday's name; `date` is a `YYYY-MM-DD` key or a Date. */
export function formatWeekday(date: string | Date, style: 'narrow' | 'short' | 'long' = 'short'): string {
  const value = typeof date === 'string' ? new Date(`${date.slice(0, 10)}T12:00`) : date
  return dateFormat({ weekday: style }).format(value)
}

/** Times read 0–23 in every language: no AM/PM, also in English. */
export const TIME_CYCLE = 'h23' as const

export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return dateFormat({
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: TIME_CYCLE,
  }).format(date)
}

/** Time only when the moment is today, date and time otherwise. */
export function formatAsOf(iso: string | null | undefined): string {
  if (!iso) return '—'
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  if (date.toDateString() !== new Date().toDateString()) return formatDateTime(iso)
  return dateFormat({ hour: '2-digit', minute: '2-digit', hourCycle: TIME_CYCLE }).format(date)
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
