import { type Time, TickMarkType } from 'lightweight-charts'

import { dateFormat } from '@/shared/lib/format'

/** Bars are UTC seconds; every label is rendered in the user's own time zone. */
const barDate = (time: Time): Date => new Date(Number(time) * 1000)

/** A bar's moment for the crosshair and the legend: a date for daily bars, date and time otherwise. */
export function formatBarTime(time: Time, minutes: number, locale: string): string {
  const daily = minutes >= 1440
  // Cached formatters: this runs on every crosshair move and every tick.
  const options: Intl.DateTimeFormatOptions = daily
    ? { day: 'numeric', month: 'short', year: 'numeric', hourCycle: 'h23' }
    : { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }
  return dateFormat(options, locale).format(barDate(time))
}

/** An axis tick at the granularity the chart asks for. */
export function formatTick(time: Time, type: TickMarkType, locale: string): string {
  const date = barDate(time)
  switch (type) {
    case TickMarkType.Year:
      return dateFormat({ year: 'numeric' }, locale).format(date)
    case TickMarkType.Month:
      return dateFormat({ month: 'short' }, locale).format(date)
    case TickMarkType.DayOfMonth:
      return dateFormat({ day: 'numeric', month: 'short' }, locale).format(date)
    default:
      return dateFormat({ hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }, locale).format(date)
  }
}

/** Price precision by magnitude: cents for big prices, eight places for tiny tokens. */
export function priceFormatFor(price: number) {
  if (price >= 1000) return { type: 'price' as const, precision: 2, minMove: 0.01 }
  if (price >= 1) return { type: 'price' as const, precision: 4, minMove: 0.0001 }
  return { type: 'price' as const, precision: 8, minMove: 1e-8 }
}
