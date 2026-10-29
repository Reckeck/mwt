import type { RouteLocationRaw } from 'vue-router'

/** The one button a thing to do gets on Home: where it leads, or the quick-add form. */
export interface TaskAction {
  /** `home.tasks.action.<label>`. */
  label: 'pay' | 'check' | 'record' | 'open' | 'sort'
  to: RouteLocationRaw | 'quick-add'
}

const ACTIONS: Record<TodayTodoKind, (todo: TodayTodo) => TaskAction> = {
  utility_overdue: (todo) => ({ label: 'pay', to: { name: 'wallet', params: { id: todo.source_id } } }),
  shortfall: () => ({ label: 'open', to: { name: 'transactions-calendar' } }),
  rejected_run: () => ({ label: 'check', to: { name: 'automations-journal' } }),
  unconfirmed_run: () => ({ label: 'check', to: { name: 'automations-journal' } }),
  queued_workflow: () => ({ label: 'check', to: { name: 'automations-journal' } }),
  reminder: () => ({ label: 'record', to: 'quick-add' }),
  pending_import: (todo) => ({ label: 'sort', to: { name: 'wallet', params: { id: todo.source_id } } }),
  points_expire: () => ({ label: 'open', to: { name: 'transactions-bonuses' } }),
  insurance_ends: (todo) => ({ label: 'open', to: { name: 'wallet', params: { id: todo.source_id } } }),
}

export function taskAction(todo: TodayTodo): TaskAction {
  return ACTIONS[todo.kind](todo)
}

/** Sums of money; `points_expire` counts points instead. */
export function isMoneyTask(kind: TodayTodoKind): boolean {
  return kind !== 'points_expire' && kind !== 'pending_import'
}

/** A thing whose date has passed: `Overdue`. A reminder for today is not late. */
export function isOverdue(todo: TodayTodo, today: string): boolean {
  return todo.date !== null && todo.date < today
}
