import { use } from 'echarts/core'
import type { ComposeOption } from 'echarts/core'
import { BarChart, LineChart, PieChart, CandlestickChart, ScatterChart, GraphChart } from 'echarts/charts'
import { GridComponent, LegendComponent, TitleComponent, TooltipComponent, DataZoomComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { type PropType, computed, defineComponent, h } from 'vue'
import VChart from 'vue-echarts'

import { type ChartColors, axisNumber, chartColors } from './chart-theme'

export type ChartOption = ComposeOption<
  | import('echarts/charts').BarSeriesOption
  | import('echarts/charts').LineSeriesOption
  | import('echarts/charts').PieSeriesOption
  | import('echarts/charts').CandlestickSeriesOption
  | import('echarts/charts').ScatterSeriesOption
  | import('echarts/charts').GraphSeriesOption
  | import('echarts/components').GridComponentOption
  | import('echarts/components').LegendComponentOption
  | import('echarts/components').TitleComponentOption
  | import('echarts/components').TooltipComponentOption
  | import('echarts/components').DataZoomComponentOption
>

use([
  BarChart,
  LineChart,
  PieChart,
  CandlestickChart,
  ScatterChart,
  GraphChart,
  GridComponent,
  LegendComponent,
  TitleComponent,
  TooltipComponent,
  DataZoomComponent,
  CanvasRenderer,
])

/** Below this chart width (px) the phone layout applies: it follows the chart, not the window. */
const NARROW = 480

/**
 * What a narrow chart changes: a legend that scrolls instead of wrapping
 * over the plot, tighter margins, axis labels that skip rather than overlap.
 * Only the parts the chart has, so a pie gets no axes.
 */
function narrowLayout(option: Record<string, unknown>): Record<string, unknown> {
  const narrow: Record<string, unknown> = {}
  // A media legend loses the base one's place (it jumped over the plot): carry it along.
  if (isObject(option.legend)) {
    const place = Object.fromEntries(
      ['top', 'bottom', 'left', 'right', 'orient']
        .filter((key) => key in (option.legend as object))
        .map((key) => [key, (option.legend as Record<string, unknown>)[key]]),
    )
    narrow.legend = { type: 'scroll', ...place }
  } else if (option.legend) narrow.legend = { type: 'scroll' }
  if (option.grid) narrow.grid = { left: 4, right: 8 }
  if (option.xAxis && !Array.isArray(option.xAxis)) narrow.xAxis = { axisLabel: { hideOverlap: true } }
  return narrow
}

/** An option with the narrow layout as an echarts media query; one that already has media stays as it is. */
export function responsiveOption(option: ChartOption): ChartOption {
  const raw = option as Record<string, unknown>
  if (raw.baseOption || raw.media) return option
  const narrow = narrowLayout(raw)
  if (!Object.keys(narrow).length) return option
  return { baseOption: option, media: [{ query: { maxWidth: NARROW }, option: narrow }] } as ChartOption
}

type Raw = Record<string, unknown>
const isObject = (value: unknown): value is Raw => typeof value === 'object' && value !== null && !Array.isArray(value)
const each = (value: unknown, fn: (item: Raw) => Raw) =>
  Array.isArray(value) ? value.map((item) => (isObject(item) ? fn(item) : item)) : isObject(value) ? fn(value) : value

/** Theme colours and readable value labels for one axis; what the chart set itself wins. */
function themedAxis(axis: Raw, colors: ChartColors): Raw {
  const label = isObject(axis.axisLabel) ? axis.axisLabel : {}
  const split = isObject(axis.splitLine) ? axis.splitLine : {}
  const line = isObject(axis.axisLine) ? axis.axisLine : {}
  const valued = axis.type === undefined || axis.type === 'value'
  return {
    ...axis,
    axisLabel: { color: colors.muted, ...(valued && !label.formatter ? { formatter: axisNumber } : {}), ...label },
    splitLine: { ...split, lineStyle: { color: colors.border, ...(isObject(split.lineStyle) ? split.lineStyle : {}) } },
    axisLine: { ...line, lineStyle: { color: colors.border, ...(isObject(line.lineStyle) ? line.lineStyle : {}) } },
  }
}

/**
 * The theme under a chart's own option: tooltip in the popover's colours (ECharts'
 * white box glared in the dark theme), the `--chart-*` palette, grid lines in
 * `--border`, compact axis numbers. Only defaults: anything the option sets stays.
 */
export function themedOption(option: Raw, colors: ChartColors): Raw {
  const out: Raw = { ...option }
  if (!out.color) out.color = colors.palette
  if (out.tooltip) {
    out.tooltip = each(out.tooltip, (tooltip) => ({
      backgroundColor: colors.popover,
      borderColor: colors.border,
      ...tooltip,
      textStyle: { color: colors.popoverText, ...(isObject(tooltip.textStyle) ? tooltip.textStyle : {}) },
    }))
  }
  for (const key of ['xAxis', 'yAxis'] as const) {
    if (out[key]) out[key] = each(out[key], (axis) => themedAxis(axis, colors))
  }
  if (isObject(out.legend)) {
    const legend = out.legend
    out.legend = {
      ...legend,
      textStyle: { color: colors.muted, ...(isObject(legend.textStyle) ? legend.textStyle : {}) },
    }
  }
  return out
}

export { VChart }

/** `VChart` in the app's theme that lays itself out for a narrow screen; events pass through. */
export const Chart = defineComponent({
  name: 'Chart',
  inheritAttrs: false,
  props: { option: { type: Object as PropType<ChartOption>, required: true } },
  setup(props, { attrs }) {
    const option = computed(() => {
      const raw = props.option as Raw
      const colors = chartColors()
      const themed = isObject(raw.baseOption)
        ? { ...raw, baseOption: themedOption(raw.baseOption, colors) }
        : themedOption(raw, colors)
      return responsiveOption(themed as ChartOption)
    })
    // `autoresize` by default: a chart created before its card had a width (a phone's
    // dashboard) drew at 700px inside 330 and stayed that way.
    return () => h(VChart, { autoresize: true, ...attrs, option: option.value })
  },
})
