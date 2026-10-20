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
    palette: [1, 2, 3, 4, 5].map((index) => tokenColor(`--chart-${index}`)),
  }
}

/** An axis value: compact (`1,2 тыс.`), one decimal at most, and never `-0`. */
export function axisNumber(value: number): string {
  const clean = Object.is(value, -0) || Math.abs(value) < 1e-9 ? 0 : value
  return numberFormat({ notation: 'compact', maximumFractionDigits: 1 }).format(clean)
}

/** An axis sum: compact with one decimal (`1,5 тыс. $`; whole thousands repeated `2 тыс.` twice), never `-0`. */
export function axisMoney(value: number, currency?: string): string {
  const clean = Object.is(value, -0) || Math.abs(value) < 1e-9 ? 0 : value
  return formatMoney(clean, { currency, compact: true, decimals: 1 })
}
