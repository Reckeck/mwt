import type { ChartOption } from '@/shared/lib/echarts'
import { axisMoney } from '@/shared/lib/chart-theme'

/** A saved scenario drawn over the current run: its balance at the end of each year. */
export interface ScenarioLine {
  name: string
  balances: number[]
}

export interface ResultChartTexts {
  year: (year: number) => string
  invested: string
  interest: string
  real: string
  crossover: string
  depleted: string
}

const num = (value: string) => Number(value)
const MONTHS_IN_YEAR = 12

/**
 * Stacked area «вложено / проценты» per year with the real balance as a
 * line; the year interest first beats the contributions and the year the
 * money runs out are marked on the axis.
 */
export function resultChart(
  result: InvestResult,
  lines: ScenarioLine[],
  texts: ResultChartTexts,
  style: { axisColor: string; primary: string; currency: string; tooltip: (params: unknown) => string },
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
    series: [
      {
        name: texts.invested,
        type: 'line',
        stack: 'balance',
        color: style.primary,
        areaStyle: { opacity: 0.35 },
        showSymbol: false,
        data: result.years.map((row) => num(row.invested)),
        markLine: marks.length
          ? {
              symbol: 'none',
              label: { formatter: '{b}', color: style.axisColor },
              lineStyle: { type: 'dashed' },
              data: marks,
            }
          : undefined,
      },
      {
        name: texts.interest,
        type: 'line',
        stack: 'balance',
        color: style.primary,
        areaStyle: { opacity: 0.15 },
        showSymbol: false,
        data: result.years.map((row) => Math.max(0, num(row.balance) - num(row.invested))),
      },
      {
        name: texts.real,
        type: 'line',
        showSymbol: false,
        color: style.axisColor,
        lineStyle: { type: 'dashed' },
        data: result.years.map((row) => num(row.real_balance)),
      },
      ...lines.map((line) => ({
        name: line.name,
        type: 'line' as const,
        showSymbol: false,
        lineStyle: { type: 'dotted' as const },
        data: labels.map((_label, index) => line.balances[index] ?? null),
      })),
    ],
  }
}
