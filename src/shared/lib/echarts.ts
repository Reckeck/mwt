import { use } from 'echarts/core'
import type { ComposeOption } from 'echarts/core'
import { BarChart, LineChart, PieChart, CandlestickChart, ScatterChart, GraphChart } from 'echarts/charts'
import { GridComponent, LegendComponent, TitleComponent, TooltipComponent, DataZoomComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { type PropType, computed, defineComponent, h } from 'vue'
import VChart from 'vue-echarts'

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
  if (option.legend) narrow.legend = { type: 'scroll' }
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

export { VChart }

/** `VChart` that lays itself out for a narrow screen; events and `autoresize` pass through. */
export const Chart = defineComponent({
  name: 'Chart',
  inheritAttrs: false,
  props: { option: { type: Object as PropType<ChartOption>, required: true } },
  setup(props, { attrs }) {
    const option = computed(() => responsiveOption(props.option))
    return () => h(VChart, { ...attrs, option: option.value })
  },
})
