import { normalizeAmount } from './amount-input'

/**
 * Arithmetic typed into an amount field (`1200+350*2`, `5000-10%`).
 *
 * Exact decimal arithmetic on bigints, never `number`; every failure is a
 * returned code, never a throw. `+ - × ÷` (also `* /`), parentheses and a
 * postfix `%`. After `+` or `-` a bare percent is relative to the left side
 * (`a+10%` = a·1.1); anywhere else it is just a hundredth (`a*10%` = a·0.1).
 */
export type AmountExprErrorCode = 'empty' | 'syntax' | 'divByZero'
export type AmountExprResult = { ok: true; value: string } | { ok: false; code: AmountExprErrorCode }

export interface AmountExprOptions {
  /** Most fraction digits kept in the result (rounded half away from zero). */
  decimals?: number
}

/** Fraction digits kept when the field sets no limit. */
const DEFAULT_DECIMALS = 18
/** Extra digits a division carries before the final rounding. */
const DIVISION_DIGITS = 30
const MAX_LENGTH = 200

interface Dec {
  int: bigint
  scale: number
}

type Node =
  | { kind: 'num'; value: Dec }
  | { kind: 'neg'; operand: Node }
  | { kind: 'pct'; operand: Node }
  | { kind: 'bin'; op: '+' | '-' | '*' | '/'; left: Node; right: Node }

type Token = { kind: 'num'; value: Dec } | { kind: 'op'; text: string }

class ExprError extends Error {
  constructor(readonly code: AmountExprErrorCode) {
    super(code)
  }
}

const pow10 = (exp: number) => 10n ** BigInt(exp)

function align(left: Dec, right: Dec): [bigint, bigint, number] {
  const scale = Math.max(left.scale, right.scale)
  return [left.int * pow10(scale - left.scale), right.int * pow10(scale - right.scale), scale]
}

function parseNumber(raw: string): Dec {
  const canonical = normalizeAmount(raw).canonical.replace(/\.$/, '')
  if (!/^\d+(\.\d+)?$/.test(canonical)) throw new ExprError('syntax')
  const [whole, fraction = ''] = canonical.split('.')
  return { int: BigInt(whole + fraction), scale: fraction.length }
}

const OPERATORS: Record<string, string> = { '×': '*', '÷': '/', '−': '-', '–': '-' }

function tokenize(text: string): Token[] {
  // «1 900» keeps its group space; any other space between digits stays a syntax error below.
  const source = text.replace(/(\d)[ \u00a0\u202f](?=\d{3}(?!\d))/g, '$1')
  const tokens: Token[] = []
  let index = 0
  while (index < source.length) {
    const char = source[index] ?? ''
    if (/\s/.test(char)) {
      index += 1
    } else if (/\d/.test(char) || ((char === '.' || char === ',') && /\d/.test(source[index + 1] ?? ''))) {
      let end = index + 1
      while (end < source.length && /[\d.,]/.test(source[end] ?? '')) end += 1
      tokens.push({ kind: 'num', value: parseNumber(source.slice(index, end)) })
      index = end
    } else {
      const op = OPERATORS[char] ?? char
      if (!'+-*/()%'.includes(op)) throw new ExprError('syntax')
      tokens.push({ kind: 'op', text: op })
      index += 1
    }
  }
  return tokens
}

function parse(tokens: Token[]): Node {
  let position = 0
  const peekOp = () => {
    const token = tokens[position]
    return token?.kind === 'op' ? token.text : null
  }

  function parseExpr(): Node {
    let left = parseTerm()
    for (let op = peekOp(); op === '+' || op === '-'; op = peekOp()) {
      position += 1
      left = { kind: 'bin', op, left, right: parseTerm() }
    }
    return left
  }
  function parseTerm(): Node {
    let left = parseUnary()
    for (let op = peekOp(); op === '*' || op === '/'; op = peekOp()) {
      position += 1
      left = { kind: 'bin', op, left, right: parseUnary() }
    }
    return left
  }
  function parseUnary(): Node {
    const op = peekOp()
    if (op === '-') {
      position += 1
      return { kind: 'neg', operand: parseUnary() }
    }
    if (op === '+') {
      position += 1
      return parseUnary()
    }
    return parsePostfix()
  }
  function parsePostfix(): Node {
    const atom = parseAtom()
    if (peekOp() === '%') {
      position += 1
      return { kind: 'pct', operand: atom }
    }
    return atom
  }
  function parseAtom(): Node {
    const token = tokens[position]
    if (!token) throw new ExprError('syntax')
    position += 1
    if (token.kind === 'num') return { kind: 'num', value: token.value }
    if (token.text !== '(') throw new ExprError('syntax')
    const inner = parseExpr()
    if (peekOp() !== ')') throw new ExprError('syntax')
    position += 1
    return inner
  }

  const tree = parseExpr()
  if (position !== tokens.length) throw new ExprError('syntax')
  return tree
}

function divide(left: Dec, right: Dec): Dec {
  if (right.int === 0n) throw new ExprError('divByZero')
  const numerator = left.int * pow10(right.scale + DIVISION_DIGITS)
  const denominator = right.int * pow10(left.scale)
  return { int: numerator / denominator, scale: DIVISION_DIGITS }
}

const HUNDRED: Dec = { int: 100n, scale: 0 }
const ONE: Dec = { int: 1n, scale: 0 }

function add(left: Dec, right: Dec): Dec {
  const [lhs, rhs, scale] = align(left, right)
  return { int: lhs + rhs, scale }
}
function sub(left: Dec, right: Dec): Dec {
  const [lhs, rhs, scale] = align(left, right)
  return { int: lhs - rhs, scale }
}
function mul(left: Dec, right: Dec): Dec {
  return { int: left.int * right.int, scale: left.scale + right.scale }
}

function evaluate(node: Node): Dec {
  switch (node.kind) {
    case 'num':
      return node.value
    case 'neg': {
      const inner = evaluate(node.operand)
      return { int: -inner.int, scale: inner.scale }
    }
    case 'pct':
      return divide(evaluate(node.operand), HUNDRED)
    case 'bin': {
      const left = evaluate(node.left)
      if ((node.op === '+' || node.op === '-') && node.right.kind === 'pct') {
        // a+10% / a-10%: a change relative to the left side.
        const share = evaluate(node.right)
        return mul(left, node.op === '+' ? add(ONE, share) : sub(ONE, share))
      }
      const right = evaluate(node.right)
      if (node.op === '+') return add(left, right)
      if (node.op === '-') return sub(left, right)
      if (node.op === '*') return mul(left, right)
      return divide(left, right)
    }
  }
}

/** `value` rounded half away from zero to `digits` places, as a plain decimal string. */
function format(value: Dec, digits: number): string {
  let { int, scale } = value
  if (scale > digits) {
    const divisor = pow10(scale - digits)
    const negative = int < 0n
    const magnitude = negative ? -int : int
    let quotient = magnitude / divisor
    if ((magnitude % divisor) * 2n >= divisor) quotient += 1n
    int = negative ? -quotient : quotient
    scale = digits
  }
  const negative = int < 0n
  const text = (negative ? -int : int).toString().padStart(scale + 1, '0')
  const whole = scale ? text.slice(0, -scale) : text
  const fraction = scale ? text.slice(-scale).replace(/0+$/, '') : ''
  const out = fraction ? `${whole}.${fraction}` : whole
  return negative && out !== '0' ? `-${out}` : out
}

export function evaluateAmountExpr(text: string, options: AmountExprOptions = {}): AmountExprResult {
  if (!text.trim()) return { ok: false, code: 'empty' }
  if (text.length > MAX_LENGTH) return { ok: false, code: 'syntax' }
  try {
    const value = evaluate(parse(tokenize(text)))
    return { ok: true, value: format(value, options.decimals ?? DEFAULT_DECIMALS) }
  } catch (error) {
    if (error instanceof ExprError) return { ok: false, code: error.code }
    return { ok: false, code: 'syntax' }
  }
}

/**
 * Whether typed text is an expression rather than a number: it holds an
 * operator, a bracket or a percent. A leading minus is a sign, not an operator.
 */
export function hasOperator(text: string): boolean {
  return /[+*/×÷()%]|[-−–]/.test(text.trim().replace(/^[-−–]/, ''))
}

/** The expression with its spaces dropped, as shown in a comment: `1200+350*2`. */
export function compactExpr(text: string): string {
  return text.replace(/\s+/g, '')
}

/** One comment line for an evaluated expression: `1200+350*2 = 1900`. */
export function noteLine(expr: string, result: string): string {
  return `${compactExpr(expr)} = ${result}`
}

/**
 * Put `line` into a comment: it replaces the field's previous line (`previous`)
 * or is appended on a new line; the rest of the comment is left as the user wrote it.
 */
export function withExpressionNote(comment: string, previous: string | null, line: string): string {
  const lines = comment ? comment.split('\n') : []
  const at = previous === null ? -1 : lines.lastIndexOf(previous)
  if (at >= 0) lines[at] = line
  else lines.push(line)
  return lines.join('\n')
}
