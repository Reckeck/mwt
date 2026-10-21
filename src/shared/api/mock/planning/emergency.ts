/** Essential spending and the emergency fund (plan C1). */

import { txLegs } from '@/shared/lib/transactions'
import { currentMonth, money, monthTotals } from './budget-calc'
import { fail } from './state'
import type { PlanningMockContext } from './state'

const MAX_FUND_MONTHS = 24

export const fund: EmergencyFund = {
  scope: { store_ids: [], include_children: false },
  target_months: 6,
  avg_months: 6,
}

const liveSavings = (ctx: PlanningMockContext) =>
  ctx.stores.filter((store) => store.purpose === 'savings' && !store.archived).map((store) => store.id)

function setFund(next: EmergencyFund, ctx: PlanningMockContext): EmergencyFund {
  for (const months of [next.target_months, next.avg_months])
    if (months < 1 || months > MAX_FUND_MONTHS) fail(`months must be 1..${MAX_FUND_MONTHS}`)
  const allowed = new Set(liveSavings(ctx))
  for (const id of next.scope.store_ids) if (!allowed.has(id)) fail(`store ${id} is not a live savings account`)
  Object.assign(fund, {
    ...next,
    scope: { store_ids: [...new Set(next.scope.store_ids)], include_children: false },
  })
  return fund
}

function toggle(storeId: string, on: boolean, ctx: PlanningMockContext): EmergencyFund {
  const ids = fund.scope.store_ids.filter((id) => id !== storeId)
  return setFund({ ...fund, scope: { ...fund.scope, store_ids: on ? [...ids, storeId] : ids } }, ctx)
}

function storeValue(storeId: string, ctx: PlanningMockContext): number {
  return ctx.transactions
    .flatMap((tx) => txLegs(tx))
    .filter((leg) => leg.role === 'principal' && leg.store_id === storeId)
    .reduce((sum, leg) => sum + Number(leg.amount) * ctx.priceOf(leg.asset.symbol), 0)
}

function status(ctx: PlanningMockContext): EmergencyStatus {
  const stores: EmergencyStore[] = fund.scope.store_ids.map((id) => ({
    store_id: id,
    value: money(storeValue(id, ctx)),
    rate: null,
    inflation: null,
    below_inflation: false,
    interest_12m: '0',
    goal_ids: [],
  }))
  const total = stores.reduce((sum, store) => sum + Number(store.value), 0)
  const month = monthTotals(ctx, currentMonth())
  const essential = month.fixed + month.essentialDaily
  const target = essential * fund.target_months
  const hasFund = stores.length > 0
  return {
    fund,
    currency: ctx.budgetCurrency,
    stores,
    total: money(total),
    e_mand: essential ? money(essential) : null,
    e_mand_months: essential ? 1 : 0,
    months_covered: hasFund && essential ? money(Math.round((total / essential) * 10) / 10) : null,
    target_amount: essential ? money(target) : null,
    surplus: hasFund && essential && total > target ? money(total - target) : null,
    interest_12m: '0',
    candidates: liveSavings(ctx).filter((id) => !fund.scope.store_ids.includes(id)),
  }
}

export function planEmergency(
  cmd: string,
  args: Record<string, unknown> | undefined,
  ctx: PlanningMockContext,
): unknown {
  switch (cmd) {
    case 'emergency_fund_get':
      return status(ctx)
    case 'emergency_fund_set':
      return setFund(args?.fund as EmergencyFund, ctx)
    case 'emergency_fund_toggle':
      return toggle(String(args?.storeId), Boolean(args?.on), ctx)
    default:
      return undefined
  }
}
