/**
 * A simplified investment engine for `pnpm dev`: monthly compounding (at the
 * end of the term for `at_end`), tax on all interest at the end. The real
 * engine is `crates/core/src/calc/invest.rs`.
 */

import { money } from './budget-calc'

const FREQ_STEP: Record<Freq, number> = { monthly: 1, quarterly: 3, yearly: 12 }

const due = (month: number, from: number, freq: Freq) => month >= from && (month - from) % FREQ_STEP[freq] === 0

function monthLabel(start: string, month: number): string {
  const [year, mon] = start.split('-').map(Number)
  const date = new Date(Date.UTC(year || 2026, (mon || 1) - 1 + month - 1, 1))
  return date.toISOString().slice(0, 7)
}

function contributionAt(input: InvestInput, month: number, timing: Timing): number {
  const stages = [...input.stages].sort((left, right) => left.from_month - right.from_month)
  const stage = stages.filter((entry) => entry.from_month <= month).at(-1)
  if (!stage || stage.timing !== timing || !due(month, stage.from_month, stage.freq)) return 0
  const years = Math.floor((month - stage.from_month) / 12)
  return Number(stage.amount) * (1 + Number(input.indexation_pct) / 100) ** years
}

export function simulate(input: InvestInput): InvestResult {
  const rate = Number(input.rate_pct) / 100
  const inflation = Number(input.inflation_pct) / 100
  const atEnd = input.capitalization === 'at_end'
  let balance = Number(input.initial)
  let accrued = 0
  let depletedAt: number | null = null
  const totals = { contributed: 0, interest: 0, tax: 0, withdrawn: 0 }
  const years: InvestYear[] = []
  let crossover: number | null = null
  for (let month = 1; month <= input.term_months; month++) {
    let contributed = contributionAt(input, month, 'start')
    balance += contributed
    const earned = (atEnd ? Number(input.initial) : balance) * (rate / 12)
    accrued += earned
    let interest = 0
    if (!atEnd || month === input.term_months) {
      interest = accrued
      balance += accrued
      accrued = 0
    }
    const endContribution = contributionAt(input, month, 'end')
    balance += endContribution
    contributed += endContribution
    let withdrawn = 0
    const plan = input.withdrawal
    if (plan && depletedAt === null && due(month, plan.from_month, plan.freq)) {
      const years = Math.floor((month - plan.from_month) / 12)
      const amount = Number(plan.amount) * (plan.index_to_inflation ? (1 + inflation) ** years : 1)
      withdrawn = Math.min(amount, balance)
      balance -= withdrawn
      if (amount >= withdrawn && balance <= 0) depletedAt = month
    }
    let tax = 0
    if (month === input.term_months) {
      tax = (totals.interest + interest) * (Number(input.tax.pct) / 100)
      balance -= tax
    }
    totals.contributed += contributed
    totals.interest += interest
    totals.tax += tax
    totals.withdrawn += withdrawn
    const yearIndex = Math.ceil(month / 12)
    let year = years[yearIndex - 1]
    if (!year) {
      year = {
        year: yearIndex,
        contributed: '0',
        interest: '0',
        tax: '0',
        withdrawn: '0',
        invested: '0',
        balance: '0',
        real_balance: '0',
        months: [],
      }
      years.push(year)
    }
    year.contributed = money(Number(year.contributed) + contributed)
    year.interest = money(Number(year.interest) + interest)
    year.tax = money(Number(year.tax) + tax)
    year.withdrawn = money(Number(year.withdrawn) + withdrawn)
    year.invested = money(Number(input.initial) + totals.contributed)
    year.balance = money(balance)
    year.real_balance = money(balance / (1 + inflation) ** (month / 12))
    year.months.push({
      month,
      date: monthLabel(input.start, month),
      contributed: money(contributed),
      interest: money(interest),
      tax: money(tax),
      withdrawn: money(withdrawn),
      balance: money(balance),
    })
  }
  for (const year of years) {
    if (crossover === null && Number(year.contributed) > 0 && Number(year.interest) > Number(year.contributed)) {
      crossover = year.year
    }
  }
  return {
    final_nominal: money(balance),
    final_real: money(balance / (1 + inflation) ** (input.term_months / 12)),
    contributed: money(totals.contributed),
    invested: money(Number(input.initial) + totals.contributed),
    interest: money(totals.interest),
    tax: money(totals.tax),
    withdrawn: money(totals.withdrawn),
    effective_rate_pct: money(((1 + rate / 12) ** 12 - 1) * 100),
    crossover_year: crossover,
    depleted_at: depletedAt,
    years,
  }
}

const finalOf = (input: InvestInput) => Number(simulate(input).final_nominal)

function scaled(input: InvestInput, factor: number): InvestInput {
  const stages = input.stages.length
    ? input.stages
    : [{ from_month: 1, amount: '1', freq: 'monthly' as Freq, timing: 'end' as Timing }]
  return { ...input, stages: stages.map((stage) => ({ ...stage, amount: money(Number(stage.amount) * factor) })) }
}

function bisect(make: (value: number) => InvestInput, low: number, high: number, target: number): number {
  let lower = low
  let upper = high
  for (let step = 0; step < 60; step++) {
    const middle = (lower + upper) / 2
    if (finalOf(make(middle)) < target) lower = middle
    else upper = middle
  }
  return upper
}

export function solve(input: InvestInput, target: number, unknown: SolveUnknown): SolveResult {
  let solved: InvestInput
  let value: number
  if (unknown === 'term') {
    const long = simulate({ ...input, term_months: 1200 })
    const hit = long.years.flatMap((year) => year.months).find((month) => Number(month.balance) >= target)
    value = hit ? hit.month : 1200
    solved = { ...input, term_months: value }
  } else if (unknown === 'rate') {
    value = bisect((rate) => ({ ...input, rate_pct: String(rate) }), 0, 200, target)
    solved = { ...input, rate_pct: money(value) }
  } else {
    let high = 1
    while (finalOf(scaled(input, high)) < target && high < 2 ** 40) high *= 2
    const factor = bisect((factor) => scaled(input, factor), 0, high, target)
    solved = scaled(input, factor)
    value = Number(solved.stages[0]?.amount ?? 0)
  }
  const result = simulate(solved)
  return { reachable: Number(result.final_nominal) >= target - 0.01, value: money(value), input: solved, result }
}

export function compare(input: InvestInput): InvestCompare {
  const base = finalOf(input)
  const moreRate = finalOf({ ...input, rate_pct: String(Number(input.rate_pct) + 2) })
  const moreContribution = finalOf(scaled({ ...input, stages: input.stages }, 1.2))
  const row = (kind: CompareKind, value: number): CompareRow => ({
    kind,
    final_nominal: money(value),
    gain: money(value - base),
  })
  return {
    rows: [
      row('base', base),
      row('more_rate', moreRate),
      row('more_contribution', input.stages.length ? moreContribution : base),
    ],
    contribution_wins: input.stages.length > 0 && moreContribution > moreRate,
  }
}

/** The balance planned after `elapsed` months: the start amount at zero. */
export function plannedBalance(input: InvestInput, elapsed: number): number {
  if (elapsed <= 0) return Number(input.initial)
  const months = simulate({ ...input, term_months: Math.min(elapsed, input.term_months) }).years.flatMap(
    (year) => year.months,
  )
  return Number(months.at(-1)?.balance ?? input.initial)
}
