import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const css = readFileSync(resolve(process.cwd(), 'src/app/styles.css'), 'utf8')

/** Variables declared in the first top-level block that opens with `selector {`. */
function readVariables(selector: string): Map<string, string> {
  const start = css.indexOf(`\n${selector} {`)
  const end = css.indexOf('\n}\n', start)
  const variables = new Map<string, string>()
  for (const match of css.slice(start, end).matchAll(/^\s+--([\w-]+):\s*([^;]+);/gm)) {
    variables.set(match[1] as string, (match[2] as string).trim())
  }
  return variables
}

function channel(hex: string, from: number): number {
  const value = Number.parseInt(hex.slice(from, from + 2), 16) / 255
  return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
}

function luminance(hex: string): number {
  return 0.2126 * channel(hex, 1) + 0.7152 * channel(hex, 3) + 0.0722 * channel(hex, 5)
}

function contrast(foreground: string, background: string): number {
  const [light, dark] = [luminance(foreground), luminance(background)].sort((first, second) => second - first)
  return ((light as number) + 0.05) / ((dark as number) + 0.05)
}

function oklchLuminance(value: string): number {
  const [lightness, chroma, hue] = (value.match(/oklch\(([^)]+)\)/)?.[1] ?? '').split(/\s+/).map(Number) as [
    number,
    number,
    number,
  ]
  const axisA = chroma * Math.cos((hue * Math.PI) / 180)
  const axisB = chroma * Math.sin((hue * Math.PI) / 180)
  const [long, medium, short] = [
    (lightness + 0.3963377774 * axisA + 0.2158037573 * axisB) ** 3,
    (lightness - 0.1055613458 * axisA - 0.0638541728 * axisB) ** 3,
    (lightness - 0.0894841775 * axisA - 1.291485548 * axisB) ** 3,
  ]
  return (
    0.2126 * (4.0767416621 * long - 3.3077115913 * medium + 0.2309699292 * short) +
    0.7152 * (-1.2684380046 * long + 2.6097574011 * medium - 0.3413193965 * short) +
    0.0722 * (-0.0041960863 * long - 0.7034186147 * medium + 1.707614701 * short)
  )
}

const themes = { light: readVariables(':root'), dark: readVariables('.dark') }
const textTokens = ['foreground', 'muted-foreground', 'success', 'destructive', 'warning']
const grounds = ['background', 'card']

describe('colour tokens', () => {
  it('defines the same set in :root and .dark, except theme-independent ones', () => {
    const shared = (name: string) => /^(safe-|port-|node-|wire-|syntax-(key|string|number|literal))/.test(name)
    const names = (theme: Map<string, string>) => [...theme.keys()].filter((name) => !shared(name)).sort()
    const light = names(themes.light)
    const dark = names(themes.dark)
    expect(dark).toEqual(light)
    for (const required of [
      'surface-inverse',
      'inverse-foreground',
      'success-soft',
      'warning-soft',
      'destructive-soft',
      'scrim',
      'text-tertiary',
      'border-strong',
      'chart-actual',
      'chart-heat-4',
    ]) {
      expect(light).toContain(required)
    }
  })

  it('exposes the new names in @theme', () => {
    for (const utility of [
      'inverse',
      'success-soft',
      'warning-soft',
      'destructive-soft',
      'scrim',
      'tertiary',
      'strong',
      'chart-actual',
      'chart-heat-4',
    ]) {
      expect(css).toMatch(new RegExp(`--color-${utility}:`))
    }
  })

  for (const [theme, variables] of Object.entries(themes)) {
    describe(theme, () => {
      for (const ground of grounds) {
        for (const token of textTokens) {
          it(`${token} on ${ground} meets AA`, () => {
            expect(contrast(variables.get(token) as string, variables.get(ground) as string)).toBeGreaterThanOrEqual(
              4.5,
            )
          })
        }
        it(`text-tertiary on ${ground} is >= 3.5:1`, () => {
          expect(
            contrast(variables.get('text-tertiary') as string, variables.get(ground) as string),
          ).toBeGreaterThanOrEqual(3.5)
        })
      }
      it('primary is ink: the same colour as the text', () => {
        expect(variables.get('primary')).toBe(variables.get('foreground'))
      })
    })
  }

  describe('workflow --port-* and --node-* (theme independent) stay visible on the canvas', () => {
    const light = themes.light
    const accents = [...light.entries()].filter(([name]) => /^(port|node)-/.test(name) && name !== 'port-flow')
    for (const [theme, variables] of Object.entries(themes)) {
      for (const [name, value] of accents) {
        it(`${name} is >= 1.8:1 against the ${theme} canvas`, () => {
          const canvas = luminance(variables.get('background') as string)
          const accent = oklchLuminance(value)
          const [high, low] = accent > canvas ? [accent, canvas] : [canvas, accent]
          expect((high + 0.05) / (low + 0.05)).toBeGreaterThanOrEqual(1.8)
        })
      }
    }
  })
})

describe('type', () => {
  it('self-hosts IBM Plex Latin and Cyrillic, only the allowed weights', () => {
    const imports = [...css.matchAll(/@import '(@fontsource\/[^']+)'/g)].map((match) => match[1]).sort()
    const sans = ['400', '500', '600']
    const expected = [
      ...sans.flatMap((weight) =>
        ['cyrillic', 'latin'].map((subset) => `@fontsource/ibm-plex-sans/${subset}-${weight}.css`),
      ),
      '@fontsource/ibm-plex-mono/cyrillic-400.css',
      '@fontsource/ibm-plex-mono/latin-400.css',
    ].sort()
    expect(imports).toEqual(expected)
  })

  it('sets the families in @theme with a system fallback', () => {
    expect(css).toMatch(/--font-sans:\s*'IBM Plex Sans',[^;]*sans-serif;/)
    expect(css).toMatch(/--font-mono:\s*'IBM Plex Mono',[^;]*monospace;/)
  })

  it('defines the scale utilities', () => {
    for (const utility of ['text-display', 'text-hero', 'text-keypad']) {
      expect(css).toContain(`@utility ${utility} {`)
    }
    expect(css).toMatch(/--text-2xs:\s*0\.6875rem;/)
  })

  it('sets the explicit radius scale in @theme and keeps no legacy --radius', () => {
    for (const [name, size] of [
      ['sm', '8px'],
      ['md', '12px'],
      ['lg', '16px'],
      ['xl', '24px'],
    ]) {
      expect(css).toMatch(new RegExp(`--radius-${name}:\\s*${size};`))
    }
    const sourceRoot = resolve(process.cwd(), 'src')
    expect(existsSync(sourceRoot)).toBe(true)
    const legacyRadius = /var\(--radius\)|calc\(var\(--radius/
    const offenders = (readdirSync(sourceRoot, { recursive: true }) as string[]).filter((file) => {
      const path = resolve(sourceRoot, file)
      return (
        !file.endsWith('styles.test.ts') && statSync(path).isFile() && legacyRadius.test(readFileSync(path, 'utf8'))
      )
    })
    expect(offenders).toEqual([])
  })
})
