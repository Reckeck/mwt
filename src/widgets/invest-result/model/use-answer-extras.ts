import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import type { CalcOutcome } from '@/features/invest-calculator'
import { call } from '@/shared/api/ipc'

/** How much longer and how much more rate the `if the contribution is unaffordable` hint tries. */
const LONGER_MONTHS = 24
const RATE_BUMP = 2

export interface ContributionAlternative {
  kind: 'term' | 'rate'
  /** Months for `term`, percent for `rate`. */
  at: number
  contribution: string
}

/**
 * What the answer adds to the run: the withdrawal that lasts the term when
 * the money runs out, and in the `Contribution` mode what a longer term or a higher
 * rate would ask for instead.
 */
export function useAnswerExtras(
  outcome: MaybeRefOrGetter<CalcOutcome | null | undefined>,
  target: MaybeRefOrGetter<string>,
) {
  const current = computed(() => toValue(outcome) ?? null)

  const depleted = computed(() => {
    const run = current.value
    return run && run.result.depleted_at !== null && run.input.withdrawal ? run.input : null
  })
  const safe = useQuery({
    queryKey: computed(() => ['calc-invest-safe', depleted.value]),
    enabled: computed(() => depleted.value !== null),
    placeholderData: keepPreviousData,
    queryFn: () => call<string | null>('calc_invest_safe_withdrawal', { input: depleted.value }),
  })

  const solving = computed(() => {
    const run = current.value
    return run?.solved?.unknown === 'contribution' && run.solved.reachable ? run.input : null
  })
  const alternatives = useQuery({
    queryKey: computed(() => ['calc-invest-alternatives', solving.value, toValue(target)]),
    enabled: computed(() => solving.value !== null),
    placeholderData: keepPreviousData,
    queryFn: async (): Promise<ContributionAlternative[]> => {
      const input = solving.value as InvestInput
      const goal = toValue(target).trim().replace(',', '.')
      const ask = (changed: InvestInput) =>
        call<SolveResult>('calc_invest_solve', { input: changed, target: goal, unknown: 'contribution' })
      const term = input.term_months + LONGER_MONTHS
      const rate = Number(input.rate_pct) + RATE_BUMP
      const [longer, richer] = await Promise.all([
        ask({ ...input, term_months: term }),
        ask({ ...input, rate_pct: String(rate) }),
      ])
      return [
        { kind: 'term', at: term, contribution: longer.value },
        { kind: 'rate', at: rate, contribution: richer.value },
      ]
    },
  })

  return {
    safeWithdrawal: computed(() => (depleted.value ? (safe.data.value ?? null) : null)),
    alternatives: computed(() => (solving.value ? (alternatives.data.value ?? []) : [])),
  }
}
