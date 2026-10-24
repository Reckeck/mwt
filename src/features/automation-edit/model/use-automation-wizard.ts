import { type InjectionKey, type Ref, computed, inject, provide, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { useStoreTree } from '@/shared/api/queries'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS, dateInputToIso, localDate } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

import {
  WIZARD_DEFAULT_TIME,
  type WizardFrequency,
  type WizardStarter,
  type WizardType,
} from '../automation-edit.constants'
import type { WizardDraft, WizardHandoff, WizardOperationForm } from '../automation-edit.types'
import { STARTER_PLANS, type StarterPlan, wizardName, wizardSchedule } from '../lib/wizard-plan'

export type WizardStep = 'what' | 'do' | 'when'
const STEPS: WizardStep[] = ['what', 'do', 'when']
const BLANK_WHEN: Omit<StarterPlan, 'kind'> = {
  frequency: 'monthly',
  day: '1',
  weekday: '0',
  workdayShift: 'none',
  asks: false,
}

/**
 * «Новая автоматизация» (Auto-1 · 6–8): what should happen → what it does →
 * when. Only a repeat goes through all three; a reminder, a price alert, a
 * scheme and two of the ready-made starts are handed on to their own forms.
 */
function createWizard(
  open: Ref<boolean>,
  handoff: (to: WizardHandoff) => void,
  created: (automation: Automation) => void,
) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const tree = useStoreTree()
  const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))
  const storeName = (id: string) => nodes.value.find((node) => node.store.id === id)?.store.name ?? ''

  const everyday = () =>
    (nodes.value.find((node) => node.store.purpose === 'everyday') ?? nodes.value[0])?.store.id ?? ''

  const step = ref<WizardStep>('what')
  const type = ref<WizardType>('repeat')
  const stepNumber = computed(() => STEPS.indexOf(step.value) + 1)

  const storeId = ref('')
  /** What the operation starts as: a ready-made start's kind, or what was typed before the wallet changed. */
  const initial = ref<WizardDraft | null>(null)
  const form = ref<WizardOperationForm | null>(null)
  /** Bumped when the form must start again: a new wallet, a ready-made start, a new opening. */
  const formKey = ref(0)
  /** The operation as it was when «Далее» left step 2. */
  const template = ref<TransactionInput | null>(null)

  const frequency = ref<WizardFrequency>('monthly')
  const day = ref('1')
  const weekday = ref('0')
  const time = ref(WIZARD_DEFAULT_TIME)
  const workdayShift = ref<WorkdayShift>('none')
  const asks = ref(false)
  const schedule = computed(() => wizardSchedule(frequency.value, day.value, weekday.value))

  // Each opening starts over, on the everyday wallet.
  watch(
    open,
    (isOpen) => {
      if (!isOpen) return
      step.value = 'what'
      type.value = 'repeat'
      initial.value = null
      template.value = null
      formKey.value += 1
      setWhen(BLANK_WHEN)
      time.value = WIZARD_DEFAULT_TIME
      storeId.value = everyday()
    },
    { immediate: true },
  )
  // Opened from a link before the wallets arrived: take the everyday one once they do.
  watch(nodes, () => {
    if (!storeId.value) storeId.value = everyday()
  })

  /** The step-3 fields: blank, or as a ready-made start sets them. */
  function setWhen(when: Omit<StarterPlan, 'kind'>) {
    frequency.value = when.frequency
    day.value = when.day
    weekday.value = when.weekday
    workdayShift.value = when.workdayShift
    asks.value = when.asks
  }

  /** A type or a start that is not a repeat closes the wizard and opens its own form. */
  function handOff(to: WizardHandoff) {
    open.value = false
    handoff(to)
  }

  function startFrom(starter: WizardStarter) {
    const plan = STARTER_PLANS[starter]
    if (!plan) return handOff({ kind: 'starter', starter: starter as 'save_share' | 'low_balance' })
    const { kind, ...when } = plan
    setWhen(when)
    type.value = 'repeat'
    initial.value = { kind, categories: [] }
    formKey.value += 1
    step.value = 'do'
  }

  /** A new wallet rebuilds the operation's form: what was typed comes along. */
  function changeStore(id: string) {
    if (form.value) initial.value = { ...form.value.draft }
    storeId.value = id
    formKey.value += 1
  }

  function goNext() {
    if (step.value === 'what') {
      if (type.value === 'repeat') step.value = 'do'
      else handOff({ kind: type.value })
      return
    }
    if (step.value === 'do') {
      form.value?.showErrors()
      if (!form.value?.valid) return
      template.value = form.value.input
      step.value = 'when'
      return
    }
    create.mutate()
  }
  function goBack() {
    step.value = STEPS[Math.max(0, stepNumber.value - 2)] ?? 'what'
  }

  /** «Ближайшие»: the next three dates, moved off days off, as the app will fire them. */
  const dates = useQuery({
    queryKey: computed(() => ['automation-dates', schedule.value, workdayShift.value, time.value]),
    queryFn: () =>
      call<string[]>('preview_automation_dates', {
        input: {
          schedule: schedule.value,
          starts_at: dateInputToIso(localDate(new Date())),
          workday_shift: workdayShift.value,
          time: time.value,
        },
      }),
    enabled: computed(() => open.value && step.value === 'when' && schedule.value !== null),
    placeholderData: keepPreviousData,
  })

  const create = useMutation({
    mutationFn: () => {
      const operation = template.value
      if (!operation || !schedule.value) throw { code: 'INVALID_INPUT', message: t('validation.summary') }
      const input: AutomationInput = {
        name: wizardName(operation, storeName, t),
        active: true,
        preset: null,
        schedule: schedule.value,
        starts_at: dateInputToIso(localDate(new Date())),
        template: operation,
        amount_mode: 'fixed',
        workday_shift: frequency.value === 'daily' ? 'none' : workdayShift.value,
        time: time.value === WIZARD_DEFAULT_TIME ? null : time.value,
        asks: asks.value,
      }
      return call<Automation>('create_automation', { input })
    },
    onSuccess: (automation) => {
      for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
      void queryClient.invalidateQueries({ queryKey: [...WORKFLOW_KEYS.list] })
      open.value = false
      created(automation)
    },
    meta: {
      error: 'automation.saveFailed',
      success: (data) => ({ text: t('automation.created'), description: (data as Automation).name }),
    },
  })

  return {
    step,
    stepNumber,
    type,
    nodes,
    storeId,
    initial,
    form,
    formKey,
    frequency,
    day,
    weekday,
    time,
    workdayShift,
    asks,
    schedule,
    dates,
    create,
    startFrom,
    changeStore,
    handOff,
    goNext,
    goBack,
  }
}

type AutomationWizard = ReturnType<typeof createWizard>
const KEY: InjectionKey<AutomationWizard> = Symbol('automation-wizard')

export function provideAutomationWizard(...args: Parameters<typeof createWizard>): AutomationWizard {
  const wizard = createWizard(...args)
  provide(KEY, wizard)
  return wizard
}

export function useAutomationWizard(): AutomationWizard {
  const wizard = inject(KEY)
  if (!wizard) throw new Error('useAutomationWizard() outside the automation wizard')
  return wizard
}
