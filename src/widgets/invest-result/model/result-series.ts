import type { LineSeriesOption } from 'echarts/charts'

import type { ResultChartStyle, ResultChartTexts, ScenarioLine } from './result-chart'

const num = (value: string) => Number(value)

/** One run alone: `Invested` and `Income` stacked, the real balance dashed, the marks on the first series. */
export function areaSeries(
  result: InvestResult,
  marks: { xAxis: string; name: string }[],
  texts: ResultChartTexts,
  style: ResultChartStyle,
): LineSeriesOption[] {
  return [
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
  ]
}

/** Runs side by side: one balance line each, the current one in `primary` over a faint area. */
export function compareSeries(
  result: InvestResult,
  lines: ScenarioLine[],
  texts: ResultChartTexts,
  style: ResultChartStyle,
): LineSeriesOption[] {
  const colors = style.palette.filter((color) => color !== style.primary)
  return [
    {
      name: texts.current,
      type: 'line',
      color: style.primary,
      areaStyle: { opacity: 0.08 },
      lineStyle: { width: 2.5 },
      showSymbol: false,
      data: result.years.map((row) => num(row.balance)),
    },
    ...lines.map<LineSeriesOption>((line, index) => ({
      name: line.name,
      type: 'line',
      color: colors[index % Math.max(1, colors.length)],
      lineStyle: { width: 2 },
      showSymbol: false,
      // A shorter plan stops where it ends; a longer one is cut at this run's last year.
      data: result.years.map((_row, year) => line.balances[year] ?? null),
      cursor: 'pointer',
      // `Clicking a line opens the calculation`: the line itself, not only its points.
      triggerLineEvent: true,
    })),
  ]
}
