import { nodeSummary } from './fields'

/**
 * A node's value in words for its card (Auto-2 · 13): `top-up to
 * Tinkoff`, `amount × 10 %`, `ask `Transfer``, `Transfer → Cash`.
 * Wallets and categories by name, a wired input by the port it comes from.
 */
export interface WordsNames {
  ru: boolean
  store: (id: string) => string
  category: (key: string) => string
}

/** Input port → the label of the output wired into it, in lower case. */
export type WiredInputs = Record<string, string>

const storeOf = (value: unknown): string | null =>
  value && typeof value === 'object' && 'store' in value ? String((value as { store: unknown }).store) : null

function optionLabel(def: NodeDef, name: string, raw: unknown, ru: boolean): string {
  const param = def.params.find((entry) => entry.name === name)
  if (param?.kind.kind !== 'select') return raw == null ? '' : String(raw)
  const option = param.kind.options.find(([key]) => key === raw)
  return option ? (ru ? option[1].ru : option[1].en) : String(raw ?? '')
}

/** `B % of A` with A and B named: a wire by its port, a field by its value. */
function operands(def: NodeDef, params: Record<string, unknown>, wired: WiredInputs, ru: boolean): string {
  const left = wired.a ?? 'A'
  const right = wired.b ?? String(params.b ?? 'B')
  if (params.op === 'percent') return `${left} × ${right} %`
  return optionLabel(def, 'op', params.op ?? def.params.find((entry) => entry.name === 'op')?.default, ru)
    .replace('A', left)
    .replace('B', right)
}

export function nodeWords(
  def: NodeDef | undefined,
  params: Record<string, unknown>,
  wired: WiredInputs,
  names: WordsNames,
  locale: string,
): string {
  if (!def) return ''
  const { ru } = names
  switch (def.kind) {
    case 'trigger.transaction': {
      const kind =
        params.tx_kind && params.tx_kind !== 'any'
          ? optionLabel(def, 'tx_kind', params.tx_kind, ru)
          : ru
            ? 'операция'
            : 'operation'
      const store = storeOf(params.store)
      const category = typeof params.category === 'string' && params.category ? names.category(params.category) : ''
      const where = store ? `${kind} ${ru ? 'на' : 'to'} ${names.store(store)}` : kind
      return category ? `${where} · ${category}` : where
    }
    case 'logic.math':
    case 'logic.compare':
      return operands(def, params, wired, ru)
    case 'action.confirm': {
      const question = String(params.question ?? '').trim()
      return question ? `${ru ? 'спросить' : 'ask'} «${question}»` : ''
    }
    case 'action.transaction': {
      const kind = optionLabel(def, 'tx_kind', params.tx_kind ?? 'transfer', ru)
      const to = storeOf(params.to)
      return to ? `${kind[0]?.toUpperCase() ?? ''}${kind.slice(1)} → ${names.store(to)}` : kind
    }
    default:
      return nodeSummary(def, params, locale)
  }
}
