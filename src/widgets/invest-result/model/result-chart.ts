import type { ChartOption } from '@/shared/lib/echarts'
import { axisMoney } from '@/shared/lib/chart-theme'

import { areaSeries, compareSeries } from './result-series'

/** Another run drawn over the current one: its balance at the end of each year. */
export interface ScenarioLine {
  name: string
  balances: number[]
}

export interface ResultChartStyle {
  axisColor: string
  primary: string
  /** Colors for the compared lines, in order; the current run keeps `primary`. */
  palette: string[]
  currency: string
  tooltip: (params: unknown) => string
}

export interface ResultChartTexts {
  year: (year: number) => string
  invested: string
  interest: string
  real: string
  crossover: string
  depleted: string
  /** «Этот расчёт»: the current run's name among the compared lines. */
  current: string
}

const MONTHS_IN_YEAR = 12

/**
 * Stacked area «вложено / проценты» per year with the real balance as a
 * line; the year interest first beats the contributions and the year the
 * money runs out are marked on the axis. With other runs to compare the
 * chart turns to one balance line per run, each in its own color (Money 4 · 22).
 */
export function resultChart(
  result: InvestResult,
  lines: ScenarioLine[],
  texts: ResultChartTexts,
  style: ResultChartStyle,
): ChartOption {
  const labels = result.years.map((row) => texts.year(row.year))
  const marks: { xAxis: string; name: string }[] = []
  if (result.crossover_year !== null) marks.push({ xAxis: texts.year(result.crossover_year), name: texts.crossover })
  if (result.depleted_at !== null) {
    marks.push({ xAxis: texts.year(Math.ceil(result.depleted_at / MONTHS_IN_YEAR)), name: texts.depleted })
  }
  return {
    backgroundColor: 'transparent',
    tooltip: { trigger: 'axis', formatter: style.tooltip },
    legend: { top: 0, left: 0, icon: 'circle', itemWidth: 8, itemHeight: 8, textStyle: { color: style.axisColor } },
    grid: { left: 8, right: 16, top: 36, bottom: 8, containLabel: true },
    xAxis: {
      type: 'category',
      data: labels,
      axisLabel: { color: style.axisColor },
      axisLine: { lineStyle: { color: style.axisColor } },
    },
    yAxis: {
      type: 'value',
      axisLabel: { color: style.axisColor, formatter: (value: number) => axisMoney(value, style.currency) },
      splitLine: { lineStyle: { color: style.axisColor, opacity: 0.15 } },
    },
    series: lines.length ? compareSeries(result, lines, texts, style) : areaSeries(result, marks, texts, style),
  }
}
