import { shallowRef } from 'vue'

import { formatMoney, numberFormat } from '@/shared/lib/format'

/** Bumped when the theme flips, so a computed that reads the colours redraws its chart. */
const revision = shallowRef(0)
let observing = false
let probe: CanvasRenderingContext2D | null | undefined

/**
 * A theme token as a colour the canvas understands. Tokens are `oklch(…)`, which
 * not every WebView's canvas parses: a 1×1 canvas paints it and reads back RGBA.
 */
function tokenColor(token: string): string {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(token).trim()
  if (probe === undefined) probe = document.createElement('canvas').getContext('2d', { willReadFrequently: true })
  if (!raw || !probe) return raw
  probe.clearRect(0, 0, 1, 1)
  probe.fillStyle = '#000'
  probe.fillStyle = raw
  probe.fillRect(0, 0, 1, 1)
  const [red, green, blue, alpha] = probe.getImageData(0, 0, 1, 1).data
  return `rgba(${red},${green},${blue},${((alpha ?? 255) / 255).toFixed(3)})`
}

export interface ChartColors {
  popover: string
  popoverText: string
  border: string
  muted: string
  /** `--primary`: the one series a chart is about. */
  primary: string
  /** `--success` and `--destructive`: a gain and a loss labelled on a line. */
  success: string
  destructive: string
  warning: string
  /** Design chart tokens: the series a chart is about, its reference line, grid, band and wash. */
  actual: string
  reference: string
  grid: string
  band: string
  wash: string
  over: string
  income: string
  /** `--chart-heat-0..4`: a calendar/heat scale from empty to full. */
  heat: string[]
  /** Category series: bank, crypto, property, accent, reference. */
  palette: string[]
}

/** The current theme's chart colours; reading them inside a computed tracks theme changes. */
export function chartColors(): ChartColors {
  if (!observing) {
    observing = true
    new MutationObserver(() => revision.value++).observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    })
  }
  void revision.value
  return {
    popover: tokenColor('--popover'),
    popoverText: tokenColor('--popover-foreground'),
    border: tokenColor('--border'),
    muted: tokenColor('--muted-foreground'),
    primary: tokenColor('--primary'),
    success: tokenColor('--success'),
    destructive: tokenColor('--destructive'),
    warning: tokenColor('--warning'),
    actual: tokenColor('--chart-actual'),
    reference: tokenColor('--chart-reference'),
    grid: tokenColor('--chart-grid'),
    band: tokenColor('--chart-band'),
    wash: tokenColor('--chart-wash'),
    over: tokenColor('--chart-over'),
    income: tokenColor('--chart-income'),
    heat: [0, 1, 2, 3, 4].map((index) => tokenColor(`--chart-heat-${index}`)),
    palette: ['bank', 'crypto', 'property', 'accent', 'reference'].map((name) => tokenColor(`--chart-${name}`)),
  }
}

/** An axis value: compact (`1.2K`), one decimal at most, and never `-0`. */
export function axisNumber(value: number): string {
  const clean = Object.is(value, -0) || Math.abs(value) < 1e-9 ? 0 : value
  return numberFormat({ notation: 'compact', maximumFractionDigits: 1 }).format(clean)
}

/** An axis sum: compact with one decimal (`1.5K $`; whole thousands repeated `2K` twice), never `-0`. */
export function axisMoney(value: number, currency?: string): string {
  const clean = Object.is(value, -0) || Math.abs(value) < 1e-9 ? 0 : value
  return formatMoney(clean, { currency, compact: true, decimals: 1 })
}
