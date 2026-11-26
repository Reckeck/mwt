// check-ui-ignore raw-color: the default surface is a colour literal fed to the contrast maths
import { NAMED_COLORS } from './named-colors'

type Rgba = { red: number; green: number; blue: number; alpha: number }

export class ColorParseError extends Error {
  readonly color: string

  constructor(color: string) {
    super(`Unsupported colour: ${color}`)
    this.name = 'ColorParseError'
    this.color = color
  }
}

export interface ContrastPair {
  foreground: string
  background: string
  min: number
}

export interface FailingPair extends ContrastPair {
  ratio: number
}

const DEFAULT_SURFACE = '#ffffff'
const FUNCTION_PATTERN = /^(rgba?|hsla?|oklch|oklab)\(\s*([^)]*?)\s*\)$/i
const HEX_PATTERN = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value))
}

function parseHex(digits: string): Rgba {
  const full = digits.length <= 4 ? [...digits].map((digit) => digit + digit).join('') : digits
  const channel = (index: number) => Number.parseInt(full.slice(index * 2, index * 2 + 2), 16)
  return {
    red: channel(0) / 255,
    green: channel(1) / 255,
    blue: channel(2) / 255,
    alpha: full.length === 8 ? channel(3) / 255 : 1,
  }
}

const NUMBER_PATTERN = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i
const ANGLE_TURNS: Record<string, number> = { deg: 1 / 360, grad: 1 / 400, rad: 1 / (2 * Math.PI), turn: 1 }

function parseNumber(token: string, color: string, percentScale = 1): number {
  if (token.toLowerCase() === 'none') return 0
  const isPercent = token.endsWith('%')
  const digits = isPercent ? token.slice(0, -1) : token
  if (!NUMBER_PATTERN.test(digits)) throw new ColorParseError(color)
  const value = Number(digits)
  return isPercent ? (value / 100) * percentScale : value
}

/** A hue in degrees; `deg`, `grad`, `rad` and `turn` are read, a bare number is degrees. */
function parseHue(token: string, color: string): number {
  const unit = /(deg|grad|rad|turn)$/i.exec(token)
  if (!unit) return parseNumber(token, color)
  return parseNumber(token.slice(0, -unit[1].length), color) * ANGLE_TURNS[unit[1].toLowerCase()] * 360
}

function parseAlpha(token: string | undefined, color: string): number {
  return token === undefined ? 1 : clamp01(parseNumber(token, color))
}

function splitArguments(body: string): { channels: string[]; alpha: string | undefined } {
  const [main, slashAlpha] = body.split('/').map((part) => part.trim())
  const parts = main.split(/[\s,]+/).filter(Boolean)
  if (slashAlpha !== undefined) return { channels: parts, alpha: slashAlpha }
  if (parts.length > 4) throw new ColorParseError(body)
  return { channels: parts.slice(0, 3), alpha: parts[3] }
}

function hslToRgb(hue: number, saturation: number, lightness: number): [number, number, number] {
  const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation
  const sector = (((hue % 360) + 360) % 360) / 60
  const second = chroma * (1 - Math.abs((sector % 2) - 1))
  const table: [number, number, number][] = [
    [chroma, second, 0],
    [second, chroma, 0],
    [0, chroma, second],
    [0, second, chroma],
    [second, 0, chroma],
    [chroma, 0, second],
  ]
  const [red, green, blue] = table[Math.min(5, Math.floor(sector))]
  const offset = lightness - chroma / 2
  return [red + offset, green + offset, blue + offset]
}

function linearToSrgb(value: number): number {
  const clamped = clamp01(value)
  return clamped <= 0.0031308 ? 12.92 * clamped : 1.055 * clamped ** (1 / 2.4) - 0.055
}

function oklabToRgb(lightness: number, axisA: number, axisB: number): [number, number, number] {
  const long = (lightness + 0.3963377774 * axisA + 0.2158037573 * axisB) ** 3
  const medium = (lightness - 0.1055613458 * axisA - 0.0638541728 * axisB) ** 3
  const short = (lightness - 0.0894841775 * axisA - 1.291485548 * axisB) ** 3
  return [
    linearToSrgb(4.0767416621 * long - 3.3077115913 * medium + 0.2309699292 * short),
    linearToSrgb(-1.2684380046 * long + 2.6097574011 * medium - 0.3413193965 * short),
    linearToSrgb(-0.0041960863 * long - 0.7034186147 * medium + 1.707614701 * short),
  ]
}

function oklchToRgb(lightness: number, chroma: number, hue: number): [number, number, number] {
  const radians = (hue * Math.PI) / 180
  return oklabToRgb(lightness, chroma * Math.cos(radians), chroma * Math.sin(radians))
}

function parseFunction(color: string, name: string, body: string): Rgba {
  const { channels, alpha } = splitArguments(body)
  if (channels.length !== 3) throw new ColorParseError(color)
  const kind = name.toLowerCase()
  if (kind.startsWith('rgb')) {
    const [red, green, blue] = channels.map((token) => clamp01(parseNumber(token, color, 255) / 255))
    return { red, green, blue, alpha: parseAlpha(alpha, color) }
  }
  if (kind.startsWith('hsl')) {
    const hue = parseHue(channels[0], color)
    const [red, green, blue] = hslToRgb(
      hue,
      clamp01(parseNumber(channels[1], color)),
      clamp01(parseNumber(channels[2], color)),
    )
    return { red, green, blue, alpha: parseAlpha(alpha, color) }
  }
  const lightness = clamp01(parseNumber(channels[0], color))
  const [red, green, blue] =
    kind === 'oklab'
      ? oklabToRgb(lightness, parseNumber(channels[1], color, 0.4), parseNumber(channels[2], color, 0.4))
      : oklchToRgb(lightness, Math.max(0, parseNumber(channels[1], color, 0.4)), parseHue(channels[2], color))
  return { red, green, blue, alpha: parseAlpha(alpha, color) }
}

function parseColor(color: string): Rgba {
  const text = color.trim()
  const hex = HEX_PATTERN.exec(text)
  if (hex) return parseHex(hex[1])
  if (text.toLowerCase() === 'transparent') return { red: 0, green: 0, blue: 0, alpha: 0 }
  const named = NAMED_COLORS[text.toLowerCase()]
  if (named) return parseHex(named.slice(1))
  const fn = FUNCTION_PATTERN.exec(text)
  if (fn) return parseFunction(color, fn[1], fn[2])
  throw new ColorParseError(color)
}

function composite(top: Rgba, bottom: Rgba): Rgba {
  const mix = (above: number, below: number) => above * top.alpha + below * (1 - top.alpha)
  return {
    red: mix(top.red, bottom.red),
    green: mix(top.green, bottom.green),
    blue: mix(top.blue, bottom.blue),
    alpha: 1,
  }
}

function luminance(color: Rgba): number {
  const linear = (value: number) => (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4)
  return 0.2126 * linear(color.red) + 0.7152 * linear(color.green) + 0.0722 * linear(color.blue)
}

/** The colour as opaque `#rrggbb` (alpha composited on the default surface); an unsupported colour is a ColorParseError. */
export function toHex(color: string): string {
  const opaque = composite(parseColor(color), { red: 1, green: 1, blue: 1, alpha: 1 })
  const byte = (value: number) =>
    Math.round(clamp01(value) * 255)
      .toString(16)
      .padStart(2, '0')
  return `#${byte(opaque.red)}${byte(opaque.green)}${byte(opaque.blue)}`
}

/** WCAG 2.x contrast ratio; alpha of the background is composited on `surface`, then the foreground on the result. */
export function contrastRatio(foreground: string, background: string, surface = DEFAULT_SURFACE): number {
  const base = composite(
    parseColor(background),
    composite(parseColor(surface), { red: 1, green: 1, blue: 1, alpha: 1 }),
  )
  const text = composite(parseColor(foreground), base)
  const [lighter, darker] = [luminance(text), luminance(base)].sort((left, right) => right - left)
  return (lighter + 0.05) / (darker + 0.05)
}

/** Returns the pairs below their minimum ratio; a missing token is a ColorParseError. */
export function checkPairs(tokens: Record<string, string>, pairs: readonly ContrastPair[]): FailingPair[] {
  const failing: FailingPair[] = []
  for (const pair of pairs) {
    const ratio = contrastRatio(tokens[pair.foreground] ?? pair.foreground, tokens[pair.background] ?? pair.background)
    if (ratio < pair.min) failing.push({ ...pair, ratio })
  }
  return failing
}
