const probe = (() => {
  if (typeof document === 'undefined') return null
  const canvas = document.createElement('canvas')
  canvas.width = 1
  canvas.height = 1
  return canvas.getContext('2d', { willReadFrequently: true })
})()

/**
 * Resolve a CSS variable to `rgba(…)`.
 *
 * The theme is written in oklch, and Chromium hands oklch/lab strings straight
 * back from both getComputedStyle and canvas fillStyle — which the chart cannot
 * parse ("Failed to parse color: lab(…)"). Painting one pixel and reading it
 * back yields plain sRGB whatever the input syntax.
 */
export function cssColor(name: string, fallback = 'rgba(128, 128, 128, 1)'): string {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  if (!probe || !raw) return fallback
  probe.clearRect(0, 0, 1, 1)
  probe.fillStyle = fallback
  probe.fillStyle = raw
  probe.fillRect(0, 0, 1, 1)
  const [red, green, blue, opacity] = probe.getImageData(0, 0, 1, 1).data
  return `rgba(${red}, ${green}, ${blue}, ${(opacity / 255).toFixed(3)})`
}

/** Scale a resolved colour's opacity; keeps the alpha it already had (borders are translucent). */
export function withAlpha(color: string, factor: number): string {
  const match = color.match(/rgba?\(([^)]+)\)/)
  if (!match) return color
  const [red, green, blue, opacity = '1'] = match[1].split(/[,\s/]+/).filter(Boolean)
  return `rgba(${red}, ${green}, ${blue}, ${(Number(opacity) * factor).toFixed(3)})`
}

/** The chart's colours from the theme tokens: candles follow gain/loss, lines the chart palette. */
export function chartPalette() {
  return {
    up: cssColor('--success'),
    down: cssColor('--destructive'),
    background: cssColor('--card'),
    text: cssColor('--muted-foreground'),
    border: cssColor('--border'),
    primary: cssColor('--primary'),
    sma: cssColor('--chart-1'),
    ema: cssColor('--chart-2'),
    rsi: cssColor('--chart-4'),
  }
}
