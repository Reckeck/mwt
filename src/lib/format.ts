import { activeLocale } from '@/locale'

/** Money always arrives as a decimal `string` from Rust (`Amount`). */
export function formatMoney(
  value: string | number | null | undefined,
  opts: { decimals?: number; currency?: string } = {},
): string {
  if (value === null || value === undefined || value === '') return '—'
  const { decimals = 2, currency } = opts
  const n = typeof value === 'string' ? Number(value) : value
  const fmt = new Intl.NumberFormat(activeLocale.value, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  })
  const body = fmt.format(n)
  return currency ? `${body} ${currency.toUpperCase()}` : body
}

export function formatPct(value: string | number | null | undefined): string {
  if (value === null || value === undefined || value === '') return '—'
  const n = typeof value === 'string' ? Number(value) : value
  return `${n.toFixed(2)}%`
}

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return '—'
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return new Intl.DateTimeFormat(activeLocale.value, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
  }).format(d)
}

export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return '—'
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return new Intl.DateTimeFormat(activeLocale.value, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(d)
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