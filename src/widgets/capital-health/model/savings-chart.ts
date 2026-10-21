import type { ChartOption } from '@/shared/lib/echarts'

export interface SavingsChartTexts {
  month: (month: string) => string
  saved: string
  target: string
}

/** The share of income saved per month, with the planned share as a dashed line. */
export function savingsChart(
  rows: SavingsMonth[],
  targetPct: string,
  texts: SavingsChartTexts,
  style: { axisColor: string; tooltip: (params: unknown) => string },
): ChartOption {
  return {
    backgroundColor: 'transparent',
    tooltip: { trigger: 'axis', formatter: style.tooltip },
    grid: { left: 8, right: 8, top: 16, bottom: 8, containLabel: true },
    xAxis: {
      type: 'category',
      data: rows.map((row) => texts.month(row.month)),
      axisLabel: { color: style.axisColor },
      axisLine: { lineStyle: { color: style.axisColor } },
    },
    yAxis: {
      type: 'value',
      axisLabel: { color: style.axisColor, formatter: '{value}%' },
      splitLine: { lineStyle: { color: style.axisColor, opacity: 0.15 } },
    },
    series: [
      {
        name: texts.saved,
        type: 'bar',
        // A month without income has no share: a gap, not a zero.
        data: rows.map((row) => (row.saved_pct === null ? null : Number(row.saved_pct))),
        markLine: {
          symbol: 'none',
          label: { formatter: texts.target, color: style.axisColor, position: 'insideEndTop' },
          lineStyle: { type: 'dashed' },
          data: [{ yAxis: Number(targetPct) }],
        },
      },
    ],
  }
}
