import { computed, onScopeDispose, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { useInflationOverview } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import {
  firstError,
  integerInRange,
  nonNegativeAmount,
  positiveAmount,
  required,
  useFormValidation,
} from '@/shared/lib/form'
import { useSettingsStore } from '@/shared/store/settings'

import { bumpAmount } from '../lib/compare-input'
import { type CalcForm, defaultForm, fromInput, newStage, termMonths, toInput } from './calc-form'

/** What the result panel shows: the run itself and, in a solving mode, the value found. */
export interface CalcOutcome {
  input: InvestInput
  result: InvestResult
  solved: { unknown: SolveUnknown; value: string; reachable: boolean } | null
}

const DEBOUNCE_MS = 300
const MAX_YEARS = 100
const MAX_RATE = 200
// The same steps as core's `Which pays more` rows.

/**
 * The calculator's state: the fields, their checks, and a live result that
 * follows them 300 ms after the last change (the previous result stays on
 * screen meanwhile, rule 17). A wrong field stops the run and shows its message.
 */
export function useInvestCalc() {
  const { t } = useI18n()
  const settings = useSettingsStore()
  // The fetched series where the currency follows it, else the flat rate.
  const { effectiveOf: inflationOf } = useInflationOverview()
  const form = reactive<CalcForm>(defaultForm(settings.baseCurrency, inflationOf(settings.baseCurrency)))

  // A new currency, or the series arriving, brings its own inflation, unless the user typed one.
  watch(
    () => [form.currency, inflationOf(form.currency)] as const,
    ([, fresh], previous) => {
      if (previous && form.inflation === previous[1]) form.inflation = fresh
    },
  )

  const above = (value: string, max: number) =>
    Number(value.trim().replace(',', '.')) > max ? t('validation.range', { min: 0, max }) : null

  const validation = useFormValidation(() => ({
    currency: required(form.currency, 'pick'),
    target: form.mode === 'result' ? null : positiveAmount(form.target),
    initial: nonNegativeAmount(form.initial),
    rate: form.mode === 'rate' ? null : firstError(nonNegativeAmount(form.rate), above(form.rate, MAX_RATE)),
    years: firstError(integerInRange(form.years, 0, MAX_YEARS), termMonths(form) > 0 ? null : t('invest.termEmpty')),
    months: integerInRange(form.months, 0, 11),
    stages: form.stages.map((stage) => nonNegativeAmount(stage.amount)).find(Boolean) ?? null,
    indexation: nonNegativeAmount(form.indexation),
    inflation: nonNegativeAmount(form.inflation),
    taxPct: firstError(nonNegativeAmount(form.taxPct), above(form.taxPct, 100)),
    withdrawal: form.withdrawal.on ? positiveAmount(form.withdrawal.amount) : null,
  }))

  /** The input the last debounce settled on; null while a field is wrong. */
  const settled = ref<{ input: InvestInput; mode: CalcForm['mode']; target: string } | null>(null)
  let timer: ReturnType<typeof setTimeout> | undefined
  watch(
    () => JSON.stringify(form),
    () => {
      clearTimeout(timer)
      const settle = () => {
        if (validation.valid.value) settled.value = { input: toInput(form), mode: form.mode, target: form.target }
        // No button to press: a wrong field shows its message once the typing stops.
        else validation.attempted.value = true
      }
      // The first answer comes at once: the result card sits above the form on a phone,
      // and an answer landing late pushed the whole form down.
      if (settled.value === null && validation.valid.value) settle()
      else timer = setTimeout(settle, DEBOUNCE_MS)
    },
    { immediate: true },
  )
  onScopeDispose(() => clearTimeout(timer))

  const outcome = useQuery({
    queryKey: computed(() => ['calc-invest', settled.value]),
    enabled: computed(() => settled.value !== null),
    placeholderData: keepPreviousData,
    queryFn: async (): Promise<CalcOutcome> => {
      const run = settled.value as NonNullable<typeof settled.value>
      if (run.mode === 'result') {
        const result = await call<InvestResult>('calc_invest_simulate', { input: run.input })
        return { input: run.input, result, solved: null }
      }
      const solved = await call<SolveResult>('calc_invest_solve', {
        input: run.input,
        target: run.target.trim().replace(',', '.'),
        unknown: run.mode,
      })
      return {
        input: solved.input,
        result: solved.result,
        solved: { unknown: run.mode, value: solved.value, reachable: solved.reachable },
      }
    },
  })

  /** Show the messages of wrong fields (the result is already live). */
  function calculate(event?: Event) {
    validation.submit(() => undefined, event)
  }

  /** Where `From your data` took the numbers from; saved with the scenario. */
  const source = ref<ScenarioSource | null>(null)
  const prefilledAt = ref<string | null>(null)
  /** The last `Refresh from data`: the answer compares a contribution with this income. */
  const prefilled = ref<ScenarioPrefill | null>(null)

  /**
   * The average income, for `that is 28 % of average income` (Money 3 · 19) before `From your data` was ever
   * run: the same backend answer with a zero share, read only, nothing goes into the fields.
   */
  const incomeHint = useQuery({
    queryKey: computed(() => ['calc-income-hint', form.currency]),
    queryFn: () =>
      call<ScenarioPrefill>('calc_scenario_prefill', {
        source: { scope: { store_ids: [], include_children: false }, income_months: 12, income_pct: '0' },
        currency: form.currency,
      }),
    placeholderData: keepPreviousData,
    enabled: computed(() => form.mode === 'contribution' && prefilled.value === null),
  })
  /** What the answer compares a found contribution with: the last `Refresh from data`, or the income alone. */
  const incomeBasis = computed(() => prefilled.value ?? incomeHint.data.value ?? null)

  /** Put a saved input into the fields, keeping the mode the user is in. */
  function load(input: InvestInput, from: ScenarioSource | null = null, at: string | null = null) {
    Object.assign(form, fromInput(input, { mode: form.mode, target: form.target }))
    source.value = from
    prefilledAt.value = at
    prefilled.value = null
  }

  /** `Refresh from data`: the balance, the deposit terms and a share of the income go into the fields. */
  function applyPrefill(from: ScenarioSource, data: ScenarioPrefill) {
    form.initial = data.initial
    if (data.rate_pct !== null) form.rate = data.rate_pct
    if (data.capitalization !== null) form.capitalization = data.capitalization
    if (data.monthly_contribution !== null) {
      const first = form.stages[0] ?? newStage()
      form.stages.splice(0, 1, { ...first, amount: data.monthly_contribution, freq: 'monthly' })
    }
    source.value = from
    prefilledAt.value = data.prefilled_at
    prefilled.value = data
  }

  /** The found value written back into the fields, so `Save` keeps what the user sees. */
  function acceptSolved() {
    const data = outcome.data.value
    if (!data?.solved) return
    Object.assign(form, fromInput(data.input, { mode: 'result', target: '' }))
  }

  /** `Use it` in `Which pays more`: the same change the row was computed with. */
  function applyCompare(kind: CompareKind) {
    if (kind === 'more_rate') form.rate = bumpAmount(form.rate, kind)
    if (kind === 'more_contribution') for (const stage of form.stages) stage.amount = bumpAmount(stage.amount, kind)
  }

  /** `Use it` the withdrawal that lasts the whole term. */
  function applyWithdrawal(amount: string) {
    form.withdrawal.amount = amount
  }

  /** What `Save` stores: the fields as the engine reads them and the `From your data` source. */
  const snapshot = () => ({ input: toInput(form), source: source.value, prefilledAt: prefilledAt.value })

  return {
    form,
    validation,
    outcome,
    source,
    prefilledAt,
    prefilled,
    incomeBasis,
    calculate,
    load,
    applyPrefill,
    acceptSolved,
    applyCompare,
    applyWithdrawal,
    snapshot,
  }
}

export type InvestCalc = ReturnType<typeof useInvestCalc>
