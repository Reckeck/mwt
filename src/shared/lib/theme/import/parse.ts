/**
 * Tokenizer and block parser for imported theme CSS (themes CI2, CI3).
 *
 * Pure, no DOM, one linear pass. Accepts exactly one `:root { --name: value; ... }` rule and
 * returns the parsed declarations. Only the parsed map leaves this module, never raw CSS text.
 * When `errors` is not empty, `declarations` is empty: nothing is applicable.
 */

export interface ParsedDeclaration {
  name: string
  value: string
  line: number
  col: number
}

export interface ParseError {
  line: number
  col: number
  message: string
}

export interface ParseResult {
  declarations: ParsedDeclaration[]
  errors: ParseError[]
}

export function formatParseError(error: ParseError): string {
  return `${error.line}:${error.col} ${error.message}`
}

const FORBIDDEN_VALUE = /(?:url|image-set|src|expression)\s*\(|javascript\s*:|behavior|-moz-binding/
const WHITESPACE = /\s/
const HEX = /[0-9a-fA-F]/

/** Decodes CSS escapes (`\75 rl(` becomes `url(`) so the checks see what a browser would see. */
export function decodeCssEscapes(text: string): string {
  let result = ''
  let index = 0
  while (index < text.length) {
    const char = text[index]
    if (char !== '\\') {
      result += char
      index += 1
      continue
    }
    index += 1
    if (index >= text.length) break
    if (!HEX.test(text[index])) {
      if (text[index] !== '\n') result += text[index]
      index += 1
      continue
    }
    let digits = ''
    while (digits.length < 6 && index < text.length && HEX.test(text[index])) {
      digits += text[index]
      index += 1
    }
    if (index < text.length && WHITESPACE.test(text[index])) index += 1
    const codePoint = Number.parseInt(digits, 16)
    const valid = codePoint > 0 && codePoint <= 0x10ffff && !(codePoint >= 0xd800 && codePoint <= 0xdfff)
    result += String.fromCodePoint(valid ? codePoint : 0xfffd)
  }
  return result
}

class Parser {
  readonly errors: ParseError[] = []
  readonly declarations: ParsedDeclaration[] = []
  private readonly text: string
  private readonly lineStarts: number[] = [0]
  private pos = 0

  constructor(source: string) {
    for (let index = 0; index < source.length; index += 1) {
      if (source[index] === '\n') this.lineStarts.push(index + 1)
    }
    this.text = this.blankComments(source)
  }

  run(): ParseResult {
    this.parseRules()
    return {
      declarations: this.errors.length === 0 ? this.declarations : [],
      errors: this.errors,
    }
  }

  private locate(offset: number): { line: number; col: number } {
    let low = 0
    let high = this.lineStarts.length - 1
    while (low < high) {
      const middle = (low + high + 1) >> 1
      if (this.lineStarts[middle] <= offset) low = middle
      else high = middle - 1
    }
    return { line: low + 1, col: offset - this.lineStarts[low] + 1 }
  }

  private fail(offset: number, message: string): void {
    this.errors.push({ ...this.locate(offset), message })
  }

  /** Replaces comments with spaces (offsets stay valid, as in CSS a comment separates tokens). */
  private blankComments(source: string): string {
    let result = ''
    let index = 0
    while (index < source.length) {
      if (source[index] === '\\') {
        result += source.slice(index, index + 2)
        index += 2
        continue
      }
      if (source.startsWith('/*', index)) {
        const end = source.indexOf('*/', index + 2)
        const stop = end === -1 ? source.length : end + 2
        if (end === -1) this.fail(index, 'unterminated comment')
        result += source.slice(index, stop).replace(/[^\n]/g, ' ')
        index = stop
        continue
      }
      result += source[index]
      index += 1
    }
    return result
  }

  private skipWhitespace(): void {
    while (this.pos < this.text.length && WHITESPACE.test(this.text[this.pos])) this.pos += 1
  }

  /** Index of the first of `stops` outside escapes before `limit`, or -1. */
  private scanTo(from: number, stops: string, limit = this.text.length): number {
    for (let index = from; index < limit; index += 1) {
      if (this.text[index] === '\\') index += 1
      else if (stops.includes(this.text[index])) return index
    }
    return -1
  }

  private parseRules(): void {
    let rules = 0
    for (;;) {
      this.skipWhitespace()
      if (this.pos >= this.text.length) {
        if (rules === 0 && this.errors.length === 0) this.fail(0, 'expected a :root block')
        return
      }
      const start = this.pos
      if (this.text[start] === '@') {
        const name = /^@[\w-]*/.exec(this.text.slice(start))?.[0] ?? '@'
        this.fail(start, `at-rule ${name} is not allowed`)
        return
      }
      const open = this.scanTo(start, '{}')
      if (open === -1 || this.text[open] === '}') {
        this.fail(open === -1 ? start : open, "malformed block: expected '{'")
        return
      }
      const selector = decodeCssEscapes(this.text.slice(start, open)).trim()
      if (rules > 0) {
        this.fail(start, 'only one rule is allowed')
        return
      }
      if (selector !== ':root') {
        this.fail(start, `selector '${selector}' is not allowed, only :root`)
        return
      }
      rules += 1
      this.pos = open + 1
      if (!this.parseBlock(open)) return
    }
  }

  /** Returns false when the block is malformed and parsing cannot continue. */
  private parseBlock(open: number): boolean {
    for (;;) {
      this.skipWhitespace()
      if (this.pos >= this.text.length) {
        this.fail(open, "malformed block: missing '}'")
        return false
      }
      if (this.text[this.pos] === '}') {
        this.pos += 1
        return true
      }
      const start = this.pos
      const stop = this.scanTo(start, ';{}')
      if (stop !== -1 && this.text[stop] === '{') {
        this.fail(stop, 'malformed block: nested block is not allowed')
        return false
      }
      const end = stop === -1 ? this.text.length : stop
      if (stop === -1) {
        this.fail(open, "malformed block: missing '}'")
        return false
      }
      this.pos = this.text[stop] === ';' ? stop + 1 : stop
      this.parseDeclaration(start, end)
    }
  }

  private parseDeclaration(start: number, end: number): void {
    const raw = this.text.slice(start, end)
    if (raw.trim() === '') return
    const colon = this.scanTo(start, ':', end)
    if (colon === -1 || colon >= end) {
      this.fail(start, `malformed declaration '${raw.trim()}': expected ':'`)
      return
    }
    const name = decodeCssEscapes(this.text.slice(start, colon)).trim()
    if (!name.startsWith('--')) {
      this.fail(start, `property '${name}' is not a custom property`)
      return
    }
    if (!/^--[\w-]+$/.test(name)) {
      this.fail(start, `invalid custom property name '${name}'`)
      return
    }
    let valueStart = colon + 1
    while (valueStart < end && WHITESPACE.test(this.text[valueStart])) valueStart += 1
    const value = this.text.slice(valueStart, end).trim()
    const problem = valueProblem(value)
    if (problem) {
      this.fail(valueStart, `${problem} in ${name}`)
      return
    }
    this.declarations.push({ name, value, ...this.locate(start) })
  }
}

function valueProblem(value: string): string | null {
  if (value === '') return 'empty value'
  const decoded = decodeCssEscapes(value).toLowerCase()
  const forbidden = FORBIDDEN_VALUE.exec(decoded)
  if (forbidden) return `'${forbidden[0].trim()}' is not allowed`
  if (/[<>;{}]|\/\*/.test(decoded)) return 'forbidden character in value'
  let depth = 0
  for (const char of decoded) {
    if (char === '(') depth += 1
    else if (char === ')') depth -= 1
    if (depth < 0) break
  }
  return depth === 0 ? null : 'unbalanced parentheses'
}

export function parseThemeCss(source: string): ParseResult {
  return new Parser(source).run()
}
