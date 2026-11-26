import type { ChartColors } from '@/shared/lib/chart-theme'
import { axisMoney } from '@/shared/lib/chart-theme'
import type { ChartOption } from '@/shared/lib/echarts'
import { kitActualSeries, kitCategoryAxis, kitGrid, kitTooltip, kitValueAxis } from '@/shared/lib/echarts-kit'

/** Capital over the months as one area line in `--chart-actual` under a wash of `--chart-wash`; a readout pinned to the top. */
export function netWorthOption(
  colors: ChartColors,
  input: { points: NetworthPoint[]; currency: string; name: string },
): ChartOption {
  const series = {
    ...kitActualSeries(colors, { name: input.name, data: input.points.map((point) => Number(point.net)) }),
    areaStyle: { color: colors.wash },
  }
  return {
    grid: kitGrid(),
    xAxis: kitCategoryAxis(
      colors,
      input.points.map((point) => point.month),
    ),
    yAxis: kitValueAxis(colors),
    tooltip: {
      ...kitTooltip(colors),
      valueFormatter: (value: unknown) => axisMoney(Number(value), input.currency),
    },
    series: [series],
  } as ChartOption
}
