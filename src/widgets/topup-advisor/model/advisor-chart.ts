import type { ChartOption } from '@/shared/lib/echarts'

export interface AdvisorChartTexts {
  target: string
  now: string
  after: string
}

/** Target, current and after-top-up share of each basket, side by side. */
export function advisorChart(
  rows: { name: string; target: number; now: number | null; after: number | null }[],
  texts: AdvisorChartTexts,
  style: { axisColor: string },
): ChartOption {
  return {
    backgroundColor: 'transparent',
    tooltip: { trigger: 'axis', valueFormatter: (value) => (value === null ? '—' : `${Number(value).toFixed(1)}%`) },
    legend: { bottom: 0, textStyle: { color: style.axisColor } },
    grid: { left: 8, right: 8, top: 16, bottom: 32, containLabel: true },
    xAxis: {
      type: 'category',
      data: rows.map((row) => row.name),
      axisLabel: { color: style.axisColor, interval: 0, overflow: 'truncate', width: 96 },
      axisLine: { lineStyle: { color: style.axisColor } },
    },
    yAxis: {
      type: 'value',
      axisLabel: { color: style.axisColor, formatter: '{value}%' },
      splitLine: { lineStyle: { color: style.axisColor, opacity: 0.15 } },
    },
    series: [
      { name: texts.target, type: 'bar', data: rows.map((row) => row.target), itemStyle: { opacity: 0.45 } },
      { name: texts.now, type: 'bar', data: rows.map((row) => row.now) },
      { name: texts.after, type: 'bar', data: rows.map((row) => row.after) },
    ],
  }
}
