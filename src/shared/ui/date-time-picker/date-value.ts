import { CalendarDate, type DateValue, getLocalTimeZone, today } from '@internationalized/date'

import type { DateTimePickerMode } from './date-time-picker.types'

const pad = (value: number) => String(value).padStart(2, '0')

/** `2026-10-14` (also the date part of `2026-10-14T09:30`) → a calendar date; anything else → undefined. */
export function toCalendarDate(value: string | null | undefined): CalendarDate | undefined {
  const match = /^(\d{4})-(\d{2})(?:-(\d{2}))?/.exec(value ?? '')
  if (!match) return undefined
  return new CalendarDate(Number(match[1]), Number(match[2]), Number(match[3] ?? 1))
}

/** The `HH:mm` part of a `datetime` value, `''` when there is none. */
export function timeOf(value: string | null | undefined): string {
  return /T(\d{2}:\d{2})/.exec(value ?? '')?.[1] ?? ''
}

/** A picked day (and time) back to the model's string. */
export function fromParts(mode: DateTimePickerMode, date: DateValue, time = ''): string {
  const day = `${date.year}-${pad(date.month)}-${pad(date.day)}`
  if (mode === 'month') return `${date.year}-${pad(date.month)}`
  if (mode === 'datetime') return `${day}T${time || '12:00'}`
  return day
}

/** «Сегодня» / «Сейчас» in the model's format. */
export function nowValue(mode: DateTimePickerMode): string {
  const now = new Date()
  return fromParts(mode, today(getLocalTimeZone()), `${pad(now.getHours())}:${pad(now.getMinutes())}`)
}

/** How the trigger reads the value in the UI language: «14 окт. 2026 г., 09:30». */
export function formatValue(mode: DateTimePickerMode, value: string, locale: string): string {
  const date = toCalendarDate(value)
  if (!date) return ''
  const js = new Date(date.year, date.month - 1, date.day)
  if (mode === 'month') return new Intl.DateTimeFormat(locale, { month: 'long', year: 'numeric' }).format(js)
  const day = new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(js)
  const time = timeOf(value)
  return mode === 'datetime' && time ? `${day}, ${time}` : day
}
