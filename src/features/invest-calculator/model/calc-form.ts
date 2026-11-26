/**
 * The calculator's fields as the form edits them (strings, years) and the
 * conversion to and from `InvestInput`, the engine's month-based input.
 */

export type CalcGoalMode = 'result' | SolveUnknown

export interface StageDraft {
  /** Year of the term the stage starts in, 1-based; the first stage always starts in year 1. */
  fromYear: string
  amount: string
  freq: Freq
  timing: Timing
}

export interface WithdrawalDraft {
  on: boolean
  amount: string
  freq: Freq
  fromYear: string
  indexed: boolean
}

export interface CalcForm {
  mode: CalcGoalMode
  /** The amount to reach in a solving mode. */
  target: string
  currency: string
  initial: string
  rate: string
  years: string
  months: string
  capitalization: CapFreq
  stages: StageDraft[]
  indexation: string
  inflation: string
  taxPct: string
  taxMode: TaxMode
  withdrawal: WithdrawalDraft
}

export const CAP_FREQS: CapFreq[] = ['monthly', 'daily', 'quarterly', 'yearly', 'at_end']
export const FREQS: Freq[] = ['monthly', 'quarterly', 'yearly']
export const GOAL_MODES: CalcGoalMode[] = ['result', 'contribution', 'term', 'rate']

const MONTHS_IN_YEAR = 12

export const newStage = (fromYear = '1'): StageDraft => ({ fromYear, amount: '', freq: 'monthly', timing: 'end' })

export function defaultForm(currency: string, inflation: string): CalcForm {
  return {
    mode: 'result',
    target: '',
    currency,
    initial: '100000',
    rate: '12',
    years: '10',
    months: '0',
    capitalization: 'monthly',
    stages: [{ ...newStage(), amount: '10000' }],
    indexation: '0',
    inflation,
    taxPct: '0',
    taxMode: 'yearly',
    withdrawal: { on: false, amount: '', freq: 'monthly', fromYear: '1', indexed: true },
  }
}

const blankZero = (value: string) => value.trim().replace(',', '.') || '0'
const whole = (value: string) => Math.max(0, Math.trunc(Number(value) || 0))
const yearToMonth = (year: string) => (Math.max(1, whole(year)) - 1) * MONTHS_IN_YEAR + 1
const monthToYear = (month: number) => String(Math.floor((Math.max(1, month) - 1) / MONTHS_IN_YEAR) + 1)

export const termMonths = (form: CalcForm) => whole(form.years) * MONTHS_IN_YEAR + whole(form.months)

/** The current month as `YYYY-MM`: the engine's calendar starts here. */
export function currentMonth(): string {
  const today = new Date()
  return `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}`
}

export function toInput(form: CalcForm): InvestInput {
  return {
    currency: form.currency,
    start: currentMonth(),
    initial: blankZero(form.initial),
    rate_pct: blankZero(form.rate),
    term_months: termMonths(form),
    capitalization: form.capitalization,
    stages: form.stages
      .filter((stage) => stage.amount.trim())
      .map((stage, index) => ({
        from_month: index === 0 ? 1 : yearToMonth(stage.fromYear),
        amount: blankZero(stage.amount),
        freq: stage.freq,
        timing: stage.timing,
      })),
    indexation_pct: blankZero(form.indexation),
    inflation_pct: blankZero(form.inflation),
    tax: { pct: blankZero(form.taxPct), mode: form.taxMode },
    withdrawal: form.withdrawal.on
      ? {
          amount: blankZero(form.withdrawal.amount),
          freq: form.withdrawal.freq,
          from_month: yearToMonth(form.withdrawal.fromYear),
          index_to_inflation: form.withdrawal.indexed,
        }
      : null,
  }
}

/** A saved input back into the fields; the mode and target stay as they are. */
export function fromInput(input: InvestInput, keep: Pick<CalcForm, 'mode' | 'target'>): CalcForm {
  const stages = [...input.stages].sort((left, right) => left.from_month - right.from_month)
  return {
    ...keep,
    currency: input.currency,
    initial: input.initial,
    rate: input.rate_pct,
    years: String(Math.floor(input.term_months / MONTHS_IN_YEAR)),
    months: String(input.term_months % MONTHS_IN_YEAR),
    capitalization: input.capitalization,
    stages: stages.length
      ? stages.map((stage) => ({
          fromYear: monthToYear(stage.from_month),
          amount: stage.amount,
          freq: stage.freq,
          timing: stage.timing,
        }))
      : [newStage()],
    indexation: input.indexation_pct,
    inflation: input.inflation_pct,
    taxPct: input.tax.pct,
    taxMode: input.tax.mode,
    withdrawal: input.withdrawal
      ? {
          on: true,
          amount: input.withdrawal.amount,
          freq: input.withdrawal.freq,
          fromYear: monthToYear(input.withdrawal.from_month),
          indexed: input.withdrawal.index_to_inflation,
        }
      : { on: false, amount: '', freq: 'monthly', fromYear: '1', indexed: true },
  }
}
