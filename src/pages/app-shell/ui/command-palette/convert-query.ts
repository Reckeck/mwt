/** Words and signs people type for a currency, beside its code: `5000 rub`, `100 $`, `20 bucks`. */
const ALIASES = new Map<string, string>([
  ['$', 'usd'],
  ['доллар', 'usd'],
  ['долл', 'usd'],
  ['бакс', 'usd'],
  ['dollar', 'usd'],
  ['₽', 'rub'],
  ['р', 'rub'],
  ['руб', 'rub'],
  ['рубль', 'rub'],
  ['рубл', 'rub'],
  ['rouble', 'rub'],
  ['ruble', 'rub'],
  ['€', 'eur'],
  ['евро', 'eur'],
  ['euro', 'eur'],
  ['₿', 'btc'],
  ['биткоин', 'btc'],
  ['бат', 'thb'],
  ['baht', 'thb'],
  ['тенге', 'kzt'],
  ['₸', 'kzt'],
  ['юань', 'cny'],
  ['¥', 'cny'],
  ['лира', 'try'],
  ['₺', 'try'],
  ['фунт', 'gbp'],
  ['£', 'gbp'],
  ['дирхам', 'aed'],
])

/** `rubles`, `dollars`, `bucks`: the endings fall off before the word is looked up. */
const ENDINGS = /(?:ей|ов|ям|ях|ами|ах|ам|я|а|ы|и|у|е|ь|s)$/u

/** `100 usd in thb`: an amount, its currency and, maybe, the one wanted. */
export interface ConvertQuery {
  amount: number
  from: string
  /** Asked for with `in thb`, `to eur`; empty for the favourites. */
  to: string
}

const PATTERN =
  /^(?<pre>[^\d\s.,]+)?\s*(?<num>\d[\d  ]*(?:[.,]\d+)?)\s*(?<cur>[^\d\s.,]+)?(?:\s+(?:в|во|на|in|to|->|→|=)\s*(?<to>[^\d\s.,]+))?$/iu

/** A typed word as a currency code, or empty: aliases first, then any code `known` accepts. */
export function currencyOf(word: string | undefined, known: (code: string) => boolean): string {
  if (!word) return ''
  const lower = word.toLowerCase()
  const alias = ALIASES.get(lower) ?? ALIASES.get(lower.replace(ENDINGS, ''))
  if (alias) return alias
  return /^[a-z]{2,6}$/.test(lower) && known(lower) ? lower : ''
}

/**
 * The palette's query read as a conversion (Currency-2 · 10), or `null` when
 * it is anything else: a number alone still searches the operations by amount.
 */
export function parseConvertQuery(text: string, known: (code: string) => boolean): ConvertQuery | null {
  const match = PATTERN.exec(text.trim())
  const groups = match?.groups
  if (!groups?.num || (groups.pre && groups.cur)) return null
  const amount = Number(groups.num.replace(/[  ]/g, '').replace(',', '.'))
  const from = currencyOf(groups.pre ?? groups.cur, known)
  if (!from || !Number.isFinite(amount) || amount <= 0) return null
  const to = currencyOf(groups.to, known)
  if (groups.to && !to) return null
  return { amount, from, to: to === from ? '' : to }
}

/** The number alone, as a spreadsheet or a form takes it: «9210.5», no spaces or mark. */
export function plainAmount(value: number, fiat: boolean): string {
  return String(Number(value.toFixed(fiat ? 2 : 8)))
}

/** The query again with another wanted currency: Tab walks through the favourites. */
export function withTarget(text: string, to: string): string {
  const base = text.trim().replace(/\s+(?:в|во|на|in|to|->|→|=)\s*[^\d\s.,]+$/iu, '')
  return `${base} в ${to}`
}
