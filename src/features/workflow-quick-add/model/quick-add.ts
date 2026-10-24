import { portAccepts } from '@/shared/lib/workflow'

/** The port the search starts from, for its heading: «Добавлена операция → сумма». */
export interface QuickAddSource {
  node: LocalizedText | string
  port: LocalizedText
}

/** Where the quick-add was opened: a free spot, a port being wired, or a wire. */
export type QuickAddContext =
  | { mode: 'free' }
  /** Dragged out of a port and dropped on empty canvas. `side` is that port's side. */
  | { mode: 'port'; nodeId: string; port: string; type: PortType; side: 'out' | 'in'; source?: QuickAddSource }
  /** Insert into a wire: the new node takes the wire's value and passes one on. */
  | { mode: 'edge'; edgeId: string; fromType: PortType; toType: PortType; source?: QuickAddSource }

/** A node to add and the port that joins it to the context. */
export interface QuickAddChoice {
  kind: string
  params: Record<string, unknown>
  /** The new node's port that joins the context (input for `out`, output for `in`). */
  port: string | null
  /** For a wire: the output that goes on to the wire's target. */
  outPort: string | null
  /** A quick expression's own title, e.g. «A > 5000». */
  hint: string
}

/** Whether a node fits the context, and through which ports. */
export function fitFor(def: NodeDef, ctx: QuickAddContext): Pick<QuickAddChoice, 'port' | 'outPort'> | null {
  if (ctx.mode === 'free') return { port: null, outPort: null }
  if (ctx.mode === 'port') {
    if (ctx.side === 'out') {
      const input = def.inputs.find((port) => portAccepts(port.type, ctx.type))
      return input ? { port: input.name, outPort: null } : null
    }
    const output = def.outputs.find((port) => portAccepts(ctx.type, port.type))
    return output ? { port: output.name, outPort: null } : null
  }
  const input = def.inputs.find((port) => portAccepts(port.type, ctx.fromType))
  const output = def.outputs.find((port) => portAccepts(ctx.toType, port.type))
  return input && output ? { port: input.name, outPort: output.name } : null
}

/** How well a node matches the search: 0 = not at all. */
export function matchScore(def: NodeDef, query: string): number {
  const needle = query.trim().toLowerCase()
  if (!needle) return 1
  const title = `${def.title.ru} ${def.title.en}`.toLowerCase()
  if (title.startsWith(needle)) return 4
  if (title.includes(needle)) return 3
  if (def.keywords.some((word) => word.toLowerCase().startsWith(needle))) return 2
  const text = `${def.description.ru} ${def.description.en} ${def.keywords.join(' ')}`.toLowerCase()
  return text.includes(needle) ? 1 : 0
}

const COMPARE: Record<string, string> = { '>': 'gt', '>=': 'gte', '<': 'lt', '<=': 'lte', '==': 'eq', '!=': 'ne' }
const MATH: Record<string, string> = { '+': 'add', '-': 'sub', '*': 'mul', '/': 'div' }

/**
 * A quick expression typed into the search: `10%` → «B % от A», `> 5000` →
 * a comparison, `+ 100` → math, `= 5000` or a bare number → a value,
 * `"text"` → a text value. `null` when the text is not one.
 */
export function quickExpression(raw: string): QuickAddChoice | null {
  const text = raw.trim()
  const number = (value: string) => value.trim().replace(',', '.').replace(/\s/g, '')
  const isNumber = (value: string) => /^-?\d+(\.\d+)?$/.test(number(value))
  const percent = /^(\d+(?:[.,]\d+)?)\s*%$/.exec(text)
  if (percent) {
    return {
      kind: 'logic.math',
      params: { op: 'percent', b: number(percent[1]) },
      port: 'a',
      outPort: 'result',
      hint: `× ${percent[1]} %`,
    }
  }
  const compare = /^(>=|<=|==|!=|>|<)\s*(-?[\d\s.,]+)$/.exec(text)
  if (compare && isNumber(compare[2])) {
    return {
      kind: 'logic.compare',
      params: { op: COMPARE[compare[1]], b: number(compare[2]) },
      port: 'a',
      outPort: 'result',
      hint: `A ${compare[1]} ${number(compare[2])}`,
    }
  }
  const math = /^([+\-*/])\s*(-?[\d\s.,]+)$/.exec(text)
  if (math && isNumber(math[2])) {
    return {
      kind: 'logic.math',
      params: { op: MATH[math[1]], b: number(math[2]), round: '2' },
      port: 'a',
      outPort: 'result',
      hint: `A ${math[1]} ${number(math[2])}`,
    }
  }
  const constant = /^=?\s*(-?[\d\s.,]+)$/.exec(text)
  if (constant && isNumber(constant[1]) && (text.startsWith('=') || isNumber(text))) {
    return {
      kind: 'data.constant',
      params: { value_type: 'number', number: number(constant[1]) },
      port: null,
      outPort: 'value',
      hint: number(constant[1]),
    }
  }
  const quoted = /^["«](.*)["»]$/.exec(text)
  if (quoted) {
    return {
      kind: 'data.constant',
      params: { value_type: 'text', text: quoted[1] },
      port: null,
      outPort: 'value',
      hint: `«${quoted[1]}»`,
    }
  }
  return null
}
