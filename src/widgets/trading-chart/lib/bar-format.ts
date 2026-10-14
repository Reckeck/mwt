import { type Time, TickMarkType } from 'lightweight-charts'

/** Bars are UTC seconds; every label is rendered in the user's own time zone. */
const barDate = (time: Time): Date => new Date(Number(time) * 1000)

/** A bar's moment for the crosshair and the legend: a date for daily bars, date and time otherwise. */
export function formatBarTime(time: Time, minutes: number, locale: string): string {
  const daily = minutes >= 1440
  return barDate(time).toLocaleString(locale, {
    day: 'numeric',
    month: 'short',
    year: daily ? 'numeric' : undefined,
    hour: daily ? undefined : '2-digit',
    minute: daily ? undefined : '2-digit',
  })
}

/** An axis tick at the granularity the chart asks for. */
export function formatTick(time: Time, type: TickMarkType, locale: string): string {
  const date = barDate(time)
  switch (type) {
    case TickMarkType.Year:
      return date.toLocaleDateString(locale, { year: 'numeric' })
    case TickMarkType.Month:
      return date.toLocaleDateString(locale, { month: 'short' })
    case TickMarkType.DayOfMonth:
      return date.toLocaleDateString(locale, { day: 'numeric', month: 'short' })
    default:
      return date.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })
  }
}

/** Price precision by magnitude: cents for big prices, eight places for tiny tokens. */
export function priceFormatFor(price: number) {
  if (price >= 1000) return { type: 'price' as const, precision: 2, minMove: 0.01 }
  if (price >= 1) return { type: 'price' as const, precision: 4, minMove: 0.0001 }
  return { type: 'price' as const, precision: 8, minMove: 1e-8 }
}
