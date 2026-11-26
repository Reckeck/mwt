import type { WealthRow } from '@/entities/report'
import { axisMoney } from '@/shared/lib/chart-theme'
import type { ChartColors } from '@/shared/lib/chart-theme'
import type { ChartOption } from '@/shared/lib/echarts'

import { GROUP_STYLE } from '../allocation-pie.constants'

/** A donut of what is owned, one slice per group in its `--chart-*` colour; debts are not a slice, they sit in the list. */
export function allocationOption(
  colors: ChartColors,
  input: { rows: WealthRow[]; currency: string; label: (row: WealthRow) => string },
): ChartOption {
  const owned = input.rows.filter((row) => row.value > 0)
  return {
    tooltip: {
      trigger: 'item',
      confine: true,
      valueFormatter: (value: unknown) => axisMoney(Number(value), input.currency),
    },
    series: [
      {
        type: 'pie',
        radius: ['58%', '88%'],
        avoidLabelOverlap: true,
        label: { show: false },
        labelLine: { show: false },
        itemStyle: { borderColor: colors.popover, borderWidth: 2 },
        data: owned.map((row) => ({
          name: input.label(row),
          value: row.value,
          itemStyle: { color: colors.palette[GROUP_STYLE[row.group].slice] },
        })),
      },
    ],
  } as ChartOption
}
