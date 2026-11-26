import { dateInputToIso, isoToDateInput } from '@/shared/lib/planning'

/** The goal form's fields as plain values (what the refs of `GoalDialog` hold). */
export interface GoalFields {
  name: string
  target: string
  currency: string
  startsAt: string
  deadline: string
  priority: number
  commitment: string
  expectedReturn: string
  storeIds: string[]
  active: boolean
  scenarioId: string | null
}

/** The fields a goal (or a draft) fills; what a goal does not say is left for the caller's default. */
export function fieldsOfGoal(goal: GoalInput | null | undefined): Partial<GoalFields> {
  if (!goal) return {}
  return {
    name: goal.name,
    target: goal.target_amount,
    currency: goal.currency,
    startsAt: isoToDateInput(goal.starts_at),
    deadline: isoToDateInput(goal.deadline),
    priority: goal.priority,
    commitment: goal.monthly_commitment ?? '',
    expectedReturn: goal.expected_return ?? '',
    storeIds: [...goal.scope.store_ids],
    active: goal.active,
    scenarioId: goal.scenario_id ?? null,
  }
}

/** The input of a goal as the core stores it: the edit form's base (a `Goal` carries an id and a date on top). */
export function goalInputOf(goal: Goal): GoalInput {
  return {
    name: goal.name,
    active: goal.active,
    target_amount: goal.target_amount,
    currency: goal.currency,
    starts_at: goal.starts_at,
    deadline: goal.deadline,
    priority: goal.priority,
    monthly_commitment: goal.monthly_commitment,
    scope: goal.scope,
    scenario_id: goal.scenario_id,
    expected_return: goal.expected_return,
  }
}

export function goalInputOfFields(fields: GoalFields): GoalInput {
  return {
    name: fields.name.trim(),
    active: fields.active,
    target_amount: fields.target.trim(),
    currency: fields.currency,
    starts_at: dateInputToIso(fields.startsAt),
    deadline: fields.deadline ? dateInputToIso(fields.deadline) : null,
    priority: fields.priority,
    monthly_commitment: fields.commitment.trim() || null,
    scope: { store_ids: fields.storeIds, include_children: true },
    scenario_id: fields.scenarioId,
    expected_return: fields.expectedReturn.trim() || null,
  }
}
