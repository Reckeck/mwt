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

/**
 * The calculator's state: the fields, their checks, and a live result that
 * follows them 300 ms after the last change (the previous result stays on
 * screen meanwhile, rule 17). A wrong field stops the run; its message shows
 * once «Рассчитать» or «Сохранить» was pressed.
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
      timer = setTimeout(() => {
        if (validation.valid.value) settled.value = { input: toInput(form), mode: form.mode, target: form.target }
      }, DEBOUNCE_MS)
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

  /** Where «Из данных» took the numbers from; saved with the scenario. */
  const source = ref<ScenarioSource | null>(null)
  const prefilledAt = ref<string | null>(null)

  /** Put a saved input into the fields, keeping the mode the user is in. */
  function load(input: InvestInput, from: ScenarioSource | null = null, at: string | null = null) {
    Object.assign(form, fromInput(input, { mode: form.mode, target: form.target }))
    source.value = from
    prefilledAt.value = at
  }

  /** «Обновить из данных»: the balance, the deposit terms and a share of the income go into the fields. */
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
  }

  /** The found value written back into the fields, so «Сохранить» keeps what the user sees. */
  function acceptSolved() {
    const data = outcome.data.value
    if (!data?.solved) return
    Object.assign(form, fromInput(data.input, { mode: 'result', target: '' }))
  }

  /** What «Сохранить» stores: the fields as the engine reads them and the «Из данных» source. */
  const snapshot = () => ({ input: toInput(form), source: source.value, prefilledAt: prefilledAt.value })

  return { form, validation, outcome, source, prefilledAt, calculate, load, applyPrefill, acceptSolved, snapshot }
}

export type InvestCalc = ReturnType<typeof useInvestCalc>
