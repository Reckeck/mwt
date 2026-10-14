import { CHART_TIMEFRAMES } from '../trading-chart.constants'

/** Inverse of `tfLabel`: "4h" → 240, "15m" → 15; unknown labels fall back to an hour. */
export function minutesOf(label: string): number {
  const match = /^(\d+)([mhd])$/.exec(label.trim())
  if (!match) return 60
  const amount = Number(match[1])
  const minutes = match[2] === 'd' ? amount * 1440 : match[2] === 'h' ? amount * 60 : amount
  return (CHART_TIMEFRAMES as readonly number[]).includes(minutes) ? minutes : 60
}

/** 60 → "1h", 240 → "4h", 1440 → "1d", anything else in minutes; also Binance's kline interval. */
export function tfLabel(minutes: number): string {
  if (minutes === 60) return '1h'
  if (minutes === 240) return '4h'
  if (minutes === 1440) return '1d'
  return `${minutes}m`
}
