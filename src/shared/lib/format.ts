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

export function formatPct(value: string | number | null | undefined): string {
  if (value === null || value === undefined || value === '') return '—'
  const numeric = typeof value === 'string' ? Number(value) : value
  return `${numeric.toFixed(2)}%`
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
    date: formatDate,
    dateTime: formatDateTime,
    locale: activeLocale,
  }
}
