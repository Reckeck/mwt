/** «Доходность +2 п.п.»: the rate the comparison and «Подставить» raise by. */
export const COMPARE_RATE_BUMP = 2
/** «Пополнения +20 %»: what every contribution is multiplied by. */
export const COMPARE_FACTOR = 1.2

const round = (value: number) => Math.round(value * 100) / 100
const parse = (value: string) => Number(value.trim().replace(',', '.')) || 0

/** A form amount raised the way a comparison variant raises it, kept as the form writes amounts. */
export function bumpAmount(value: string, kind: Exclude<CompareKind, 'base'>): string {
  const number = parse(value)
  return String(round(kind === 'more_rate' ? number + COMPARE_RATE_BUMP : number * COMPARE_FACTOR))
}

/** The plan of a «Что выгоднее» variant: the same input with the rate or the contributions raised. */
export function compareInput(input: InvestInput, kind: CompareKind): InvestInput {
  if (kind === 'more_rate') return { ...input, rate_pct: bumpAmount(input.rate_pct, kind) }
  if (kind === 'more_contribution') {
    return { ...input, stages: input.stages.map((stage) => ({ ...stage, amount: bumpAmount(stage.amount, kind) })) }
  }
  return input
}
