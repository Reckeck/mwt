/** The planning entry point: tries each group of commands. */

import { planAllocation } from './allocation'
import { planAutomationAmounts } from './automation-workflow'
import { planAutomations } from './automations'
import { upcoming } from './budget-calc'
import { planCalculator } from './calculator'
import { planEmergency } from './emergency'
import { planHealth } from './health'
import { planReminders } from './reminders'
import { mockContext } from './state'
import type { PlanningMockContext } from './state'
import { planToday } from './today'
import { planUtilities } from './utilities'

/**
 * Answer a planning command, or `undefined` when `cmd` is not one.
 * `settings` supplies the savings rate and the daily budget mode.
 */
export function mockPlanning(
  cmd: string,
  args: Record<string, unknown> | undefined,
  ctx: PlanningMockContext,
  settings: Settings,
): unknown {
  mockContext.value = ctx
  if (cmd === 'get_upcoming') return upcoming(Number(args?.days ?? 30))
  return (
    planAutomations(cmd, args, ctx, settings) ??
    planAutomationAmounts(cmd, args, ctx) ??
    planReminders(cmd, args, ctx, settings) ??
    planUtilities(cmd, args, ctx, settings) ??
    planCalculator(cmd, args, ctx) ??
    planEmergency(cmd, args, ctx) ??
    planHealth(cmd, args, ctx, settings) ??
    planAllocation(cmd, args, ctx) ??
    planToday(cmd, args, ctx, settings)
  )
}
