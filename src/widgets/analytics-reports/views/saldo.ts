import type { RouteLocationRaw } from 'vue-router'

import { drillTo } from '@/shared/lib/drill'
import type { ChartOption } from '@/shared/lib/echarts'

import { categoryChart, num } from './chart'

/** Series names of the two charts, already translated. */
export interface SaldoLabels {
  left: string
  forecast: string
  even: string
  daily: string
  limit: string
}

/** The transactions of one day, all wallets (the balance counts them all). */
export function transactionsOfDay(date: string): RouteLocationRaw {
  return drillTo({ from: date })
}

/**
 * What each day was allowed: the pool over the month on the 1st, then what the
 * day before left for every day after it (the forecast's flat share ahead).
 */
export function dayLimits(month: CalendarMonth): (number | null)[] {
  const pool = month.budget === null ? null : num(month.budget) / month.days.length
  return month.days.map((_day, index) => {
    if (index === 0) return pool
    const before = month.days[index - 1].per_day_left
    return before === null ? null : num(before)
  })
}

/** The balance day by day: real, forecast ahead (joined to the last real day), and spending evenly. */
export function balanceChart(
  month: CalendarMonth,
  labels: string[],
  axisColor: string,
  tooltip: (params: unknown) => string,
  names: SaldoLabels,
): ChartOption {
  const days = month.days
  const lastReal = days.reduce((last, day, index) => (day.forecast ? last : index), -1)
  const pool = num(month.budget)
  const left = (day: CalendarDay) => (day.left === null ? null : num(day.left))
  return {
    ...categoryChart(labels, axisColor, tooltip, month.currency),
    series: [
      {
        name: names.left,
        type: 'line',
        showSymbol: false,
        areaStyle: { opacity: 0.12 },
        data: days.map((day) => (day.forecast ? null : left(day))),
      },
      {
        name: names.forecast,
        type: 'line',
        showSymbol: false,
        lineStyle: { type: 'dashed' },
        data: days.map((day, index) =>
          day.forecast || (index === lastReal && lastReal < days.length - 1) ? left(day) : null,
        ),
      },
      // An even pace only means something while there is a pool to spend.
      ...(pool > 0
        ? [
            {
              name: names.even,
              type: 'line' as const,
              showSymbol: false,
              lineStyle: { type: 'dotted' as const, width: 1 },
              data: days.map((_day, index) => Math.round((pool * (days.length - index - 1)) / days.length)),
            },
          ]
        : []),
    ],
  }
}

/** Everyday spending of each day against what the day was allowed. */
export function spendingChart(
  month: CalendarMonth,
  labels: string[],
  axisColor: string,
  tooltip: (params: unknown) => string,
  names: SaldoLabels,
): ChartOption {
  return {
    ...categoryChart(labels, axisColor, tooltip, month.currency),
    series: [
      { name: names.daily, type: 'bar', data: month.days.map((day) => (day.forecast ? null : num(day.daily))) },
      {
        name: names.limit,
        type: 'line',
        step: 'middle',
        showSymbol: false,
        lineStyle: { type: 'dashed' },
        data: dayLimits(month),
      },
    ],
  }
}
