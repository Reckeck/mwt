import type { ChartOption } from '@/shared/lib/echarts'
import { axisMoney } from '@/shared/lib/chart-theme'

/**
 * Axes, grid and tooltip every report chart shares; `labels` is the x axis.
 * A money chart passes `tooltip` (`useMoneyChartTooltip`): each value then also
 * shows in the base and favourite currencies. With `currency` the value axis
 * reads as money («12K ₽»), so a chart is never a column of bare numbers.
 * On a narrow screen `Chart` scrolls the legend and thins the labels.
 */
export function categoryChart(
  labels: string[],
  axisColor: string,
  tooltip?: (params: unknown) => string,
  currency?: string,
): ChartOption {
  return {
    backgroundColor: 'transparent',
    tooltip: tooltip ? { trigger: 'axis', formatter: tooltip } : { trigger: 'axis' },
    legend: { bottom: 0, textStyle: { color: axisColor } },
    grid: { left: 16, right: 16, top: 16, bottom: 40, containLabel: true },
    xAxis: {
      type: 'category',
      data: labels,
      axisLabel: { color: axisColor },
      axisLine: { lineStyle: { color: axisColor } },
    },
    yAxis: {
      type: 'value',
      axisLabel: currency
        ? {
            color: axisColor,
            formatter: (value: number) => axisMoney(value, currency),
          }
        : { color: axisColor },
      splitLine: { lineStyle: { color: axisColor, opacity: 0.15 } },
    },
  }
}

export const num = (value: string | null | undefined) => Number(value ?? 0)
