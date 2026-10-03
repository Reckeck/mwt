import { use } from 'echarts/core'
import type { ComposeOption } from 'echarts/core'
import { LineChart, PieChart } from 'echarts/charts'
import {
  GridComponent,
  LegendComponent,
  TitleComponent,
  TooltipComponent,
} from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { provide, type InjectionKey } from 'vue'
import VChart from 'vue-echarts'

export type ChartOption = ComposeOption<
  | import('echarts/charts').LineSeriesOption
  | import('echarts/charts').PieSeriesOption
  | import('echarts/components').GridComponentOption
  | import('echarts/components').LegendComponentOption
  | import('echarts/components').TitleComponentOption
  | import('echarts/components').TooltipComponentOption
>

use([
  LineChart,
  PieChart,
  GridComponent,
  LegendComponent,
  TitleComponent,
  TooltipComponent,
  CanvasRenderer,
])

const THEME_KEY: InjectionKey<string> = Symbol('echarts-theme')

export function provideChartTheme(theme: 'light' | 'dark') {
  provide(THEME_KEY, theme)
}

export { VChart }
export const Chart = VChart