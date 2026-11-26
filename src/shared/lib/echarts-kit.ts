import type { ChartColors } from './chart-theme'
import type { ChartOption } from './echarts'

/** A value axis never spans more than this: a non-finite or runaway bound froze the axis at 1e13. */
const AXIS_LIMIT = 1e12

const guardBound = (value: number): number =>
  Number.isFinite(value) ? Math.max(-AXIS_LIMIT, Math.min(AXIS_LIMIT, value)) : 0

/** The chart's one value axis: hairline grid in `--chart-grid`, fitted to the data (`scale`) with guarded bounds. */
export function kitValueAxis(colors: ChartColors): NonNullable<ChartOption['yAxis']> {
  return {
    type: 'value',
    scale: true,
    splitNumber: 4,
    min: (range: { min: number }) => guardBound(range.min),
    max: (range: { max: number }) => guardBound(range.max),
    axisLine: { show: false },
    axisTick: { show: false },
    splitLine: { show: true, lineStyle: { color: colors.grid, width: 1 } },
  }
}

/** The category (time) axis: no grid lines, a hairline baseline. */
export function kitCategoryAxis(colors: ChartColors, categories: string[]): NonNullable<ChartOption['xAxis']> {
  return {
    type: 'category',
    data: categories,
    boundaryGap: false,
    axisTick: { show: false },
    axisLine: { lineStyle: { color: colors.grid, width: 1 } },
    splitLine: { show: false },
  }
}

export function kitGrid(): NonNullable<ChartOption['grid']> {
  return { top: 16, right: 12, bottom: 8, left: 8, containLabel: true }
}

/** A readout pinned to the top edge of the plot, following the pointer sideways only; a dashed pointer line. */
export function kitTooltip(colors: ChartColors): NonNullable<ChartOption['tooltip']> {
  return {
    trigger: 'axis',
    axisPointer: { type: 'line', lineStyle: { color: colors.reference, type: 'dashed', width: 1 } },
    confine: true,
    position: (
      point: number[],
      _params: unknown,
      _dom: unknown,
      _rect: unknown,
      size: { contentSize: number[]; viewSize: number[] },
    ) => [Math.max(0, Math.min(point[0] - size.contentSize[0] / 2, size.viewSize[0] - size.contentSize[0])), 0],
  }
}

/** The ECharts legend, shown only when there are at least two series to tell apart. */
export function kitLegend(colors: ChartColors, names: string[]): NonNullable<ChartOption['legend']> {
  return {
    show: names.length >= 2,
    data: names,
    top: 0,
    right: 0,
    icon: 'circle',
    itemWidth: 8,
    itemHeight: 8,
    textStyle: { color: colors.muted },
  }
}

export interface MarkerPoint {
  /** Category of the x axis the marker sits on. */
  at: string
  value: number
  label?: string
}

/** `markPoint` data: a ring in the series colour on the picked points (a peak, today, an event). */
export function kitMarkers(colors: ChartColors, points: MarkerPoint[]) {
  return {
    symbol: 'circle',
    symbolSize: 8,
    itemStyle: { color: colors.popover, borderColor: colors.actual, borderWidth: 2 },
    label: { show: false },
    data: points.map((point) => ({ name: point.label ?? point.at, coord: [point.at, point.value] })),
  }
}

export interface KitSeries {
  name: string
  data: number[]
}

/** The series a chart is about: a solid line in `--chart-actual`, markers on request. */
export function kitActualSeries(colors: ChartColors, series: KitSeries, markers: MarkerPoint[] = []) {
  return {
    type: 'line' as const,
    name: series.name,
    data: series.data,
    showSymbol: false,
    lineStyle: { color: colors.actual, width: 2 },
    itemStyle: { color: colors.actual },
    ...(markers.length ? { markPoint: kitMarkers(colors, markers) } : {}),
  }
}

/** The line to compare against (a plan, last year): dashed, `--chart-reference`, not hoverable. */
export function kitReferenceSeries(colors: ChartColors, series: KitSeries) {
  return {
    type: 'line' as const,
    name: series.name,
    data: series.data,
    showSymbol: false,
    silent: true,
    lineStyle: { color: colors.reference, width: 1.5, type: 'dashed' as const },
    itemStyle: { color: colors.reference },
  }
}

export interface KitLineInput {
  categories: string[]
  actual: KitSeries
  reference?: KitSeries
  markers?: MarkerPoint[]
}

/** A time-series line chart in the kit: one value axis, hairline grid, pinned readout, optional reference and markers. */
export function kitLineOption(colors: ChartColors, input: KitLineInput): ChartOption {
  const series = [
    kitActualSeries(colors, input.actual, input.markers),
    ...(input.reference ? [kitReferenceSeries(colors, input.reference)] : []),
  ]
  return {
    grid: kitGrid(),
    xAxis: kitCategoryAxis(colors, input.categories),
    yAxis: kitValueAxis(colors),
    tooltip: kitTooltip(colors),
    legend: kitLegend(
      colors,
      series.map((item) => item.name),
    ),
    series,
  } as ChartOption
}
