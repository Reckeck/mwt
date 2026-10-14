import { type InjectionKey, inject, provide } from 'vue'

import type { TransactionFormProps } from '../transaction-form.types'
import { useTxAutofill } from './use-tx-autofill'
import { useTxBudget } from './use-tx-budget'
import { useTxFills } from './use-tx-fills'
import { useTxFormState } from './use-tx-form-state'
import { useTxKind } from './use-tx-kind'
import { useTxPreview } from './use-tx-preview'
import { useTxPricing } from './use-tx-pricing'
import { useTxSave } from './use-tx-save'
import { useTxValidation } from './use-tx-validation'

/**
 * The whole operation form, put together once by `TransactionForm` and handed
 * to its sections: they read the same `form`, `check` and helpers instead of a
 * dozen props each. The order matters only where a later part watches an
 * earlier one (the kind before the assets it starts with).
 */
function setup(props: TransactionFormProps, onSaved: (tx: Transaction) => void) {
  const state = useTxFormState(props)
  return {
    ...state,
    ...useTxKind(state),
    ...useTxAutofill(state),
    ...useTxFills(state),
    ...useTxPricing(state),
    ...useTxBudget(state),
    ...useTxPreview(state),
    ...useTxSave(state, onSaved),
    ...useTxValidation(state),
  }
}

export type TxFormContext = ReturnType<typeof setup>

const TX_FORM: InjectionKey<TxFormContext> = Symbol('tx-form')

/** Build the form and make it available to the sections below. */
export function provideTxForm(props: TransactionFormProps, onSaved: (tx: Transaction) => void): TxFormContext {
  const context = setup(props, onSaved)
  provide(TX_FORM, context)
  return context
}

/** The form a section belongs to; only inside `TransactionForm`. */
export function useTxForm(): TxFormContext {
  const context = inject(TX_FORM)
  if (!context) throw new Error('useTxForm() outside TransactionForm')
  return context
}
