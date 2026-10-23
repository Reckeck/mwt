/** Saved calculations of the demo vault: something to compare the current run with (Money 4 · 22). */

import { scenarios } from './calculator'
import { iso, now, uuid } from './state'

const monthly = (amount: string): ContributionStage => ({ from_month: 1, amount, freq: 'monthly', timing: 'end' })

function input(initial: string, rate: string, years: number, amount: string): InvestInput {
  const today = now()
  return {
    currency: 'USD',
    start: `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}`,
    initial,
    rate_pct: rate,
    term_months: years * 12,
    capitalization: 'monthly',
    stages: amount === '0' ? [] : [monthly(amount)],
    indexation_pct: '0',
    inflation_pct: '3.74',
    tax: { pct: '0', mode: 'yearly' },
    withdrawal: null,
  }
}

const SAVED: [name: string, input: InvestInput][] = [
  ['Пенсия', input('20000', '8', 20, '300')],
  ['Машина 2029', input('2000', '10', 3, '450')],
  ['Подушка', input('3000', '6', 2, '200')],
]

export function seedScenarios() {
  const stamp = iso(now())
  for (const [name, saved] of SAVED) {
    scenarios.push({
      id: uuid(),
      name,
      input: saved,
      source: null,
      prefilled_at: null,
      created_at: stamp,
      updated_at: stamp,
    })
  }
}
