import { activeLocale } from '@/shared/lib/locale'

/** Money always arrives as a decimal `string` from Rust (`Amount`). */
export function formatMoney(
  value: string | number | null | undefined,
  opts: { decimals?: number; currency?: string } = {},
): string {
  if (value === null || value === undefined || value === '') return '—'
  const { decimals = 2, currency } = opts
  const numeric = typeof value === 'string' ? Number(value) : value
  const fmt = new Intl.NumberFormat(activeLocale.value, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  })
  const body = fmt.format(numeric)
  return currency ? `${body} ${currency.toUpperCase()}` : body
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
    money: (value: string | number | null | undefined, opts?: { decimals?: number; currency?: string }) =>
      formatMoney(value, opts),
    formatPct,
    date: formatDate,
    dateTime: formatDateTime,
    locale: activeLocale,
  }
}
