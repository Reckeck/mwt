/**
 * The formula with the typed numbers in place of its words:
 * «(показание − предыдущее) × 3,5» → «(209 − 205) × 3,5». Display only; the
 * amount itself comes from `preview_utility_formula`.
 */
export function fillFormula(
  formula: string,
  values: { current: string; previous: string; used: string; tariff: string },
) {
  const words = new Map([
    ['показание', values.current],
    ['предыдущее', values.previous],
    ['расход', values.used],
    ['тариф', values.tariff],
  ])
  return formula.replace(/[\p{L}_][\p{L}\p{N}_]*/gu, (word) => words.get(word.toLowerCase().replace(/ё/g, 'е')) ?? word)
}

/** The formula as typed, with × and − as the canvas prints them. */
export const prettyFormula = (formula: string) =>
  formula
    .replace(/\*/g, '×')
    .replace(/(\s)-(\s)/g, '$1−$2')
    .replace(/(\d)\.(\d)/g, '$1,$2')
