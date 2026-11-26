// check-ui-ignore raw-color: test fixtures, arbitrary colour values
import { describe, expect, it } from 'vitest'

import type { ChartColors } from './chart-theme'
import { kitLineOption, kitMarkers, kitReferenceSeries, kitTooltip, kitValueAxis } from './echarts-kit'

// biome-ignore lint/suspicious/noExplicitAny: a probe into an option object the test only reads
type Probe = Record<string, any>

const colors = {
  popover: '#001',
  popoverText: '#002',
  border: '#003',
  muted: '#004',
  actual: '#005',
  reference: '#006',
  grid: '#007',
} as ChartColors

const input = {
  categories: ['a', 'b', 'c'],
  actual: { name: 'Actual', data: [1, 2, 3] },
  reference: { name: 'Plan', data: [2, 2, 2] },
  markers: [{ at: 'b', value: 2 }],
}

describe('chart kit option helpers', () => {
  it('builds one value axis with the grid colour token', () => {
    const option = kitLineOption(colors, input) as Probe
    const axes = [option.xAxis, option.yAxis].flat().filter((axis) => axis.type === 'value')
    expect(axes).toHaveLength(1)
    expect(axes[0].splitLine.lineStyle.color).toBe(colors.grid)
    expect(axes[0].splitLine.lineStyle.width).toBe(1)
  })

  it('guards the value axis against non-finite and runaway bounds', () => {
    const axis = kitValueAxis(colors) as Probe
    expect(axis.scale).toBe(true)
    expect(axis.max({ min: 0, max: Number.POSITIVE_INFINITY })).toBe(0)
    expect(axis.max({ min: 0, max: 1e13 })).toBe(1e12)
    expect(axis.min({ min: Number.NaN, max: 1 })).toBe(0)
    expect(axis.min({ min: 5, max: 9 })).toBe(5)
  })

  it('shows the legend only for two or more series', () => {
    const two = kitLineOption(colors, input) as Probe
    const one = kitLineOption(colors, { ...input, reference: undefined }) as Probe
    expect(two.legend.show).toBe(true)
    expect(two.legend.data).toEqual(['Actual', 'Plan'])
    expect(one.legend.show).toBe(false)
  })

  it('pins the readout to the top edge, clamped inside the chart', () => {
    const tooltip = kitTooltip(colors) as Probe
    const size = { contentSize: [100, 40], viewSize: [300, 200] }
    expect(tooltip.trigger).toBe('axis')
    expect(tooltip.position([10, 90], null, null, null, size)).toEqual([0, 0])
    expect(tooltip.position([150, 90], null, null, null, size)).toEqual([100, 0])
    expect(tooltip.position([290, 90], null, null, null, size)).toEqual([200, 0])
  })

  it('marks the picked points and draws the reference dashed', () => {
    const option = kitLineOption(colors, input) as Probe
    expect(option.series[0].markPoint.data).toEqual([{ name: 'b', coord: ['b', 2] }])
    expect(kitMarkers(colors, []).data).toEqual([])
    const reference = kitReferenceSeries(colors, input.reference)
    expect(reference.lineStyle.type).toBe('dashed')
    expect(reference.lineStyle.color).toBe(colors.reference)
  })
})
