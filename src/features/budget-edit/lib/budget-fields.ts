import type { BudgetTargetKind } from '../budget-edit.constants'

/** The limit form's fields as plain values (what the refs of `BudgetDialog` hold). */
export interface BudgetFields {
  targetKind: BudgetTargetKind
  category: string
  storeId: string
  period: BudgetPeriod
  limit: string
  currency: string
  warn: boolean
  active: boolean
}

/** The fields a stored limit fills in the form. */
export function fieldsOfBudget(budget: BudgetInput): BudgetFields {
  const target = budget.target
  return {
    targetKind: target === 'total' ? 'total' : 'category' in target ? 'category' : 'store',
    category: typeof target === 'object' && 'category' in target ? target.category : '',
    storeId: typeof target === 'object' && 'store' in target ? (target.store.store_ids[0] ?? '') : '',
    period: budget.period,
    limit: budget.limit,
    currency: budget.currency,
    warn: budget.warn ?? true,
    active: budget.active,
  }
}

/** The input of a limit as the core stores it: the edit form's base (a `Budget` carries an id on top). */
export function budgetInputOf(budget: Budget): BudgetInput {
  return {
    active: budget.active,
    target: budget.target,
    period: budget.period,
    limit: budget.limit,
    currency: budget.currency,
    warn: budget.warn,
  }
}

export function budgetInputOfFields(fields: BudgetFields): BudgetInput {
  const target: BudgetTarget =
    fields.targetKind === 'total'
      ? 'total'
      : fields.targetKind === 'category'
        ? { category: fields.category }
        : { store: { store_ids: [fields.storeId], include_children: true } }
  return {
    active: fields.active,
    target,
    period: fields.period,
    limit: fields.limit.trim(),
    currency: fields.currency.trim().toLowerCase(),
    warn: fields.warn,
  }
}
