/** The meter formula for `pnpm dev`: the grammar of `utilities_formula.rs`, on plain numbers. */

const VARIABLES: [string, string][] = [
  ['показание', 'current'],
  ['предыдущее', 'previous'],
  ['расход', 'used'],
  ['тариф', 'tariff'],
  ['дней', 'days'],
]
const FUNCTIONS = new Map([
  ['min', 'min'],
  ['мин', 'min'],
  ['max', 'max'],
  ['макс', 'max'],
  ['round', 'round'],
  ['округл', 'round'],
  ['if', 'if'],
  ['если', 'if'],
])

const zoneKey = (zone: string) => zone.trim().toLowerCase().split(/\s+/).join('_')

class FormulaError extends Error {
  constructor(
    readonly at: number,
    readonly ru: string,
    readonly en: string,
  ) {
    super(en)
  }
}

type Token =
  | { kind: 'num'; value: number; at: number }
  | { kind: 'name'; value: string; at: number }
  | { kind: 'sign'; value: string; at: number }

function tokenize(source: string): Token[] {
  const out: Token[] = []
  const pattern = /\s+|(\d+(?:[.,]\d+)?)|([\p{L}_][\p{L}\p{N}_]*)|(>=|<=|<>|!=|[-+*/:()<>=;−×])/uy
  while (pattern.lastIndex < source.length) {
    const at = pattern.lastIndex
    const match = pattern.exec(source)
    if (!match) throw new FormulaError(at, `непонятный знак «${source[at]}»`, `unexpected sign «${source[at]}»`)
    if (match[1]) out.push({ kind: 'num', value: Number(match[1].replace(',', '.')), at })
    else if (match[2]) out.push({ kind: 'name', value: match[2].toLowerCase().replace(/ё/g, 'е'), at })
    else if (match[3]) out.push({ kind: 'sign', value: match[3], at })
  }
  return out
}

/** Recursive descent straight to a number: comparison > sum > product > unary > primary. */
function evaluate(source: string, vars: Record<string, number>): number {
  if (!source.trim()) throw new FormulaError(0, 'формула пустая', 'the formula is empty')
  const tokens = tokenize(source)
  let index = 0
  const peek = () => tokens[index]
  const isSign = (...signs: string[]) => peek()?.kind === 'sign' && signs.includes(String(peek()?.value))
  const expect = (sign: string, at: number) => {
    if (!isSign(sign)) throw new FormulaError(at, 'скобка не закрыта', 'the bracket is not closed')
    index += 1
  }
  const comparison = (): number => {
    const left = sum()
    if (!isSign('>', '<', '>=', '<=', '=', '<>', '!=')) return left
    const sign = String(tokens[index++].value)
    const right = sum()
    const result = {
      '>': left > right,
      '<': left < right,
      '>=': left >= right,
      '<=': left <= right,
      '=': left === right,
    }[sign]
    return (result ?? left !== right) ? 1 : 0
  }
  const sum = (): number => {
    let left = product()
    while (isSign('+', '-', '−')) left = tokens[index++].value === '+' ? left + product() : left - product()
    return left
  }
  const product = (): number => {
    let left = unary()
    while (isSign('*', '×', '/', ':')) {
      const token = tokens[index++]
      const right = unary()
      if ((token.value === '/' || token.value === ':') && right === 0) {
        throw new FormulaError(token.at, 'деление на ноль', 'division by zero')
      }
      left = token.value === '/' || token.value === ':' ? left / right : left * right
    }
    return left
  }
  const unary = (): number => {
    if (isSign('-', '−')) {
      index += 1
      return -unary()
    }
    if (isSign('+')) {
      index += 1
      return unary()
    }
    return primary()
  }
  const primary = (): number => {
    const token = tokens[index++]
    if (!token) throw new FormulaError(source.length, 'формула оборвалась', 'the formula ends too early')
    if (token.kind === 'num') return token.value
    if (token.kind === 'sign' && token.value === '(') {
      const inner = comparison()
      expect(')', token.at)
      return inner
    }
    if (token.kind === 'name' && isSign('(')) {
      const open = tokens[index++]
      const args: number[] = []
      if (!isSign(')')) {
        args.push(comparison())
        while (isSign(';')) {
          index += 1
          args.push(comparison())
        }
      }
      expect(')', open.at)
      const fn = FUNCTIONS.get(token.value)
      if (!fn)
        throw new FormulaError(token.at, `неизвестная функция «${token.value}»`, `unknown function «${token.value}»`)
      if (fn === 'if') return args[0] ? args[1] : args[2]
      if (fn === 'round') return Number(args[0].toFixed(Math.max(0, Math.min(10, args[1] ?? 0))))
      return fn === 'min' ? Math.min(...args) : Math.max(...args)
    }
    if (token.kind === 'name') {
      if (token.value in vars) return vars[token.value]
      throw new FormulaError(token.at, `неизвестное имя «${token.value}»`, `unknown name «${token.value}»`)
    }
    throw new FormulaError(token.at, 'здесь ждали число или имя', 'a number or a name was expected here')
  }
  const result = comparison()
  if (index < tokens.length)
    throw new FormulaError(tokens[index].at, 'здесь не хватает знака действия', 'a sign is missing here')
  return result
}

function variables(input: UtilityFormulaInput): Record<string, number> {
  const vars: Record<string, number> = {}
  const put = (suffix: string, previous: number, current: number) => {
    const values = [current, previous, current - previous, Number(input.tariff), input.days ?? 30]
    VARIABLES.forEach(([ru, en], index) => {
      vars[`${ru}${suffix}`] = values[index]
      vars[`${en}${suffix}`] = values[index]
    })
  }
  const of = (zone: string | null) =>
    input.readings.find(
      (reading) => (reading.zone === null ? null : zoneKey(reading.zone)) === (zone === null ? null : zoneKey(zone)),
    )
  const zoned = input.zones.map((zone) => ({ zone, reading: of(zone) }))
  const total = input.zones.length
    ? zoned.reduce(
        (sum, { reading }) => ({
          previous: sum.previous + Number(reading?.previous ?? 0),
          current: sum.current + Number(reading?.current ?? 0),
        }),
        { previous: 0, current: 0 },
      )
    : { previous: Number(of(null)?.previous ?? 0), current: Number(of(null)?.current ?? 0) }
  put('', total.previous, total.current)
  for (const { zone, reading } of zoned)
    put(`_${zoneKey(zone)}`, Number(reading?.previous ?? 0), Number(reading?.current ?? 0))
  return vars
}

/** `preview_utility_formula`. */
export function previewFormula(input: UtilityFormulaInput): UtilityFormulaPreview {
  const vars = variables(input)
  const consumption = String(vars.used)
  const formula = input.formula?.trim()
  if (!formula)
    return { amount: (Math.round(vars.used * Number(input.tariff) * 100) / 100).toFixed(2), consumption, issue: null }
  try {
    return { amount: (Math.round(evaluate(formula, vars) * 100) / 100).toFixed(2), consumption, issue: null }
  } catch (error) {
    if (!(error instanceof FormulaError)) throw error
    return { amount: null, consumption, issue: { position: error.at + 1, text_ru: error.ru, text_en: error.en } }
  }
}

/** A meter's bill for one month's readings per zone (`null` zone for a one-rate meter). */
export function meterBill(item: UtilityItem, readings: FormulaZoneReading[]): number {
  if (!('metered' in item.kind)) return 0
  const { tariff, formula, zones } = item.kind.metered
  return Number(previewFormula({ tariff, formula, zones, readings, days: null }).amount ?? 0)
}
