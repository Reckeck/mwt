/**
 * Validation of an imported theme file (themes CI1, CI4-CI6, CI8, CI9).
 *
 * Pure, no DOM. Works on the parsed declaration map from `parse.ts` only: raw CSS text never leaves
 * this module. `theme` is set only when `errors` is empty, so nothing is applied on an error.
 * Font properties get no special case (CI6): `font-family` is a parse error (not a custom property),
 * `--font-sans` is an unknown custom property, hence a dropped warning.
 */
import { ColorParseError, type FailingPair, toHex } from '../contrast'
import { defaultDark } from '../themes/default-dark'
import { defaultLight } from '../themes/default-light'
import { THEME_TOKENS, type ThemeToken } from '../tokens'
import type { ThemeMode, ThemeTokens } from '../types'
import { type ParsedDeclaration, parseThemeCss } from './parse'

export const MAX_FILE_BYTES = 64 * 1024
export const MAX_DECLARATIONS = 256
export const MAX_VALUE_LENGTH = 512
const MAX_RADIUS_PX = 32
const DEFAULT_NAME = 'Custom theme'
/** Relative luminance above which a background reads as a light theme (equal contrast to black and white). */
const LIGHT_LUMINANCE = 0.179

export interface ValidationIssue {
  line: number
  col: number
  message: string
}

export interface ValidatedTheme {
  name: string
  mode: ThemeMode
  /** True when the mode was not declared in the file and was guessed from the background luminance. */
  modeGuessed: boolean
  tokens: ThemeTokens
}

export interface ValidationResult {
  errors: ValidationIssue[]
  warnings: ValidationIssue[]
  theme?: ValidatedTheme
  /** Filled by the contrast step (CI7); empty here. */
  contrast: FailingPair[]
}

const TOKEN_SET: ReadonlySet<string> = new Set(THEME_TOKENS)
const FORBIDDEN_FUNCTION = /\b(var|env|calc)\s*\(/i
const HEX_COLOR = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i
const COLOR_FUNCTION = /^(rgba?|hsla?|oklch|oklab)\(([^()]*)\)$/i
const COLOR_ARGUMENT = /^(?:none|[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?(?:%|deg|rad|grad|turn)?)$/i
const LENGTH = /^(\d*\.?\d+)(px|rem)?$/i
const NAMED_COLORS: ReadonlySet<string> = new Set(
  (
    'transparent aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown ' +
    'burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod ' +
    'darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon ' +
    'darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray ' +
    'dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green ' +
    'greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon ' +
    'lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon ' +
    'lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta ' +
    'maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen ' +
    'mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive ' +
    'olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru ' +
    'pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen ' +
    'seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato ' +
    'turquoise violet wheat white whitesmoke yellow yellowgreen'
  ).split(' '),
)

function isColor(value: string): boolean {
  const text = value.trim()
  if (HEX_COLOR.test(text) || NAMED_COLORS.has(text.toLowerCase())) return true
  const match = COLOR_FUNCTION.exec(text)
  if (!match) return false
  const parts = match[2].split(/[\s,/]+/).filter(Boolean)
  return (parts.length === 3 || parts.length === 4) && parts.every((part) => COLOR_ARGUMENT.test(part))
}

function radiusProblem(value: string): string | null {
  const match = LENGTH.exec(value.trim())
  if (!match) return 'expected a length in px or rem'
  const amount = Number(match[1])
  const unit = match[2]?.toLowerCase()
  if (!unit && amount !== 0) return 'expected a length in px or rem'
  const pixels = unit === 'rem' ? amount * 16 : amount
  return pixels > MAX_RADIUS_PX ? 'radius must be between 0 and 2rem' : null
}

function valueProblem(token: ThemeToken, value: string): string | null {
  if (token === '--radius') return radiusProblem(value)
  return isColor(value) ? null : 'expected a colour (#hex, rgb, hsl, oklch, oklab or a named colour)'
}

function readComment(header: string, key: 'name' | 'mode'): string | undefined {
  for (const comment of header.matchAll(/\/\*([\s\S]*?)\*\//g)) {
    const found = new RegExp(`^\\s*${key}\\s*:\\s*(.+?)\\s*$`, 'im').exec(comment[1])
    if (found) return found[1]
  }
  return undefined
}

function nameFromFile(fileName: string | undefined): string {
  const base = fileName
    ?.split(/[\\/]/)
    .pop()
    ?.replace(/\.css$/i, '')
    .trim()
  return base || DEFAULT_NAME
}

function guessMode(background: string | undefined): ThemeMode {
  if (!background) return 'dark'
  try {
    const hex = toHex(background)
    const linear = (channel: number) => (channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4)
    const [red, green, blue] = [1, 3, 5].map((offset) =>
      linear(Number.parseInt(hex.slice(offset, offset + 2), 16) / 255),
    )
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue > LIGHT_LUMINANCE ? 'light' : 'dark'
  } catch (error) {
    if (error instanceof ColorParseError) return 'dark'
    throw error
  }
}

function declarationProblem({ name, value }: ParsedDeclaration): string | null {
  if (value.length > MAX_VALUE_LENGTH) return `value of ${name} is longer than ${MAX_VALUE_LENGTH} characters`
  const forbidden = FORBIDDEN_FUNCTION.exec(value)
  if (forbidden) return `${forbidden[1].toLowerCase()}() is not allowed in ${name}`
  if (!TOKEN_SET.has(name)) return null
  const problem = valueProblem(name as ThemeToken, value)
  return problem ? `${problem} in ${name}` : null
}

/** Validates the text of a custom theme file; `fileName` feeds the name fallback (Q6). */
export function validateTheme(text: string, fileName?: string): ValidationResult {
  const errors: ValidationIssue[] = []
  const warnings: ValidationIssue[] = []
  const result = (theme?: ValidatedTheme): ValidationResult => ({ errors, warnings, theme, contrast: [] })

  if (new TextEncoder().encode(text).length > MAX_FILE_BYTES) {
    errors.push({ line: 1, col: 1, message: `file is larger than ${MAX_FILE_BYTES / 1024} KiB` })
    return result()
  }
  const parsed = parseThemeCss(text)
  if (parsed.errors.length > 0) {
    errors.push(...parsed.errors)
    return result()
  }
  if (parsed.declarations.length > MAX_DECLARATIONS) {
    errors.push({ line: 1, col: 1, message: `more than ${MAX_DECLARATIONS} declarations` })
    return result()
  }

  const values = new Map<ThemeToken, string>()
  for (const declaration of parsed.declarations) {
    const problem = declarationProblem(declaration)
    if (problem) errors.push({ line: declaration.line, col: declaration.col, message: problem })
    if (TOKEN_SET.has(declaration.name)) values.set(declaration.name as ThemeToken, declaration.value)
    else
      warnings.push({
        line: declaration.line,
        col: declaration.col,
        message: `unknown token ${declaration.name} is ignored`,
      })
  }
  if (errors.length > 0) return result()

  const header = text.slice(0, text.indexOf('{'))
  const declaredMode = readComment(header, 'mode')?.toLowerCase()
  const modeDeclared = declaredMode === 'light' || declaredMode === 'dark'
  if (declaredMode !== undefined && !modeDeclared) {
    warnings.push({ line: 1, col: 1, message: `mode '${declaredMode}' is not light or dark and is ignored` })
  }
  const mode: ThemeMode = modeDeclared ? declaredMode : guessMode(values.get('--background'))
  const base = mode === 'light' ? defaultLight : defaultDark

  const missing = THEME_TOKENS.filter((token) => !values.has(token))
  if (missing.length > 0) {
    warnings.push({ line: 1, col: 1, message: `missing tokens fall back to ${base.name}: ${missing.join(', ')}` })
  }
  const tokens = Object.fromEntries(THEME_TOKENS.map((token) => [token, values.get(token) ?? base.tokens[token]]))
  return result({
    name: readComment(header, 'name') ?? nameFromFile(fileName),
    mode,
    modeGuessed: !modeDeclared,
    tokens: tokens as ThemeTokens,
  })
}
