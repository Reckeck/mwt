import { type Ref, computed } from 'vue'

import type { StoreCreateStep } from '../store-create.types'

/**
 * The wizard's steps (Wallets-1 · 2–4). A top-level wallet: what → how it is
 * called → what is there now; one without an opening balance (property, a
 * loan) and a sub-account stop after the second. An exchange's sub-account is
 * one step: its type is a field there (Wallets-2 · 9).
 */
export function useWizardSteps(
  step: Ref<StoreCreateStep>,
  isSub: Ref<boolean>,
  offersOpening: Ref<boolean>,
  presetKind: () => boolean,
  oneStep: Ref<boolean>,
) {
  const steps = computed<StoreCreateStep[]>(() => {
    if (isSub.value && oneStep.value) return ['details']
    return isSub.value || !offersOpening.value ? ['pick', 'details'] : ['pick', 'details', 'opening']
  })
  const stepNumber = computed(() => steps.value.indexOf(step.value) + 1)
  const isLast = computed(() => stepNumber.value === steps.value.length)
  /** A preset kind (the sidebar group's «+») has no step 1 to return to. */
  const canGoBack = computed(() => stepNumber.value > (presetKind() && !isSub.value ? 2 : 1))

  function goBack() {
    step.value = steps.value[Math.max(0, stepNumber.value - 2)] ?? 'pick'
  }
  function goNext() {
    step.value = steps.value[stepNumber.value] ?? step.value
  }

  return { steps, stepNumber, isLast, goBack, goNext, canGoBack }
}
