/**
 * Browser-mock half of planning: automations, budgets, goals, reminders,
 * alerts and the agenda for `pnpm dev`. The numbers are rough stand-ins for
 * clicking through the UI; the real rules (funds check, carry-over, shares
 * of shared stores) live only in the backend.
 */

export type { PlanningMockContext } from './state'
export { seedUtilityDemo } from './utility-calc'
export * from './dispatch'
