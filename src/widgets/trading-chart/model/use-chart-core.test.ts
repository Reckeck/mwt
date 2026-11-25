import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, ref, shallowRef } from 'vue'

import { defaultDark } from '@/shared/lib/theme/themes/default-dark'
import { nord } from '@/shared/lib/theme/themes/nord'

const theme = shallowRef(defaultDark)
const palette = vi.fn(() => ({
  background: '#000000',
  text: '#ffffff',
  border: '#111111',
  primary: '#222222',
  up: '#00ff00',
  down: '#ff0000',
}))

vi.mock('@/shared/lib/theme', async () => {
  const { computed } = await import('vue')
  return { useTheme: () => ({ theme, resolved: computed(() => theme.value.mode) }) }
})
vi.mock('../lib/chart-color', () => ({ chartPalette: () => palette(), withAlpha: (color: string) => color }))
vi.mock('lightweight-charts', () => {
  const series = () => ({ applyOptions: vi.fn(), setData: vi.fn(), update: vi.fn() })
  return {
    ColorType: { Solid: 'solid' },
    CrosshairMode: { Normal: 0 },
    CandlestickSeries: 'candles',
    HistogramSeries: 'histogram',
    createChart: () => ({
      addSeries: series,
      applyOptions: vi.fn(),
      priceScale: () => ({ applyOptions: vi.fn() }),
      subscribeCrosshairMove: vi.fn(),
      remove: vi.fn(),
    }),
  }
})

import { useChartCore } from './use-chart-core'

describe('useChartCore theme', () => {
  it('re-applies the palette when the theme changes within one mode', async () => {
    const Host = defineComponent({
      setup() {
        const el = ref<HTMLDivElement | null>(null)
        useChartCore({
          el,
          symbol: () => 'BTCUSDT',
          limit: () => 10,
          tf: ref(60),
          indicators: ref([]),
          onReady: () => undefined,
        })
        return () => h('div', { ref: el })
      },
    })
    mount(Host)
    const callsAfterMount = palette.mock.calls.length
    expect(nord.mode).toBe(defaultDark.mode)
    theme.value = nord
    await nextTick()
    expect(palette.mock.calls.length).toBeGreaterThan(callsAfterMount)
  })
})
