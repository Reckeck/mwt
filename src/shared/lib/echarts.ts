import { use } from 'echarts/core'
import type { ComposeOption } from 'echarts/core'
import { BarChart, LineChart, PieChart, CandlestickChart, ScatterChart, GraphChart } from 'echarts/charts'
import { GridComponent, LegendComponent, TitleComponent, TooltipComponent, DataZoomComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
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

export { VChart }
export const Chart = VChart
