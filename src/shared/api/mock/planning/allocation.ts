/** Portfolio baskets and the top-up helper (plan F): a simplified copy of `service/allocation.rs`. */

import { assetKey } from '@/shared/lib/asset-ref'
import { txLegs } from '@/shared/lib/transactions'
import { money, monthTotals } from './budget-calc'
import { fund } from './emergency'
import { fail, now, uuid } from './state'
import type { PlanningMockContext } from './state'

const PRESET: AssetClass[] = [
  { id: 'local', name: '', target_pct: '40', cap: false },
  { id: 'global', name: '', target_pct: '40', cap: false },
  { id: 'risky', name: '', target_pct: '20', cap: true },
]
const OUTSIDE = new Set(['property', 'loyalty', 'liability'])

const saved: AssetClass[] = []
const links: AssetClassLink[] = []

const effective = () => (saved.length ? saved : PRESET)
const view = (): AssetClassesView => ({ classes: effective(), preset: !saved.length, links: [...links] })

function setClasses(classes: AssetClass[], ctx: PlanningMockContext): AssetClassesView {
  const ids = new Set<string>()
  const out = classes.map((row) => {
    const next = { ...row, name: row.name.trim(), id: row.id.trim() || uuid() }
    const pct = Number(next.target_pct)
    if (pct < 0 || pct > 100) fail(`asset class ${next.id}: target share must be 0..=100`)
    if (ids.has(next.id)) fail(`asset class ${next.id}`, 'ALREADY_EXISTS')
    ids.add(next.id)
    return next
  })
  const total = out.reduce((sum, row) => sum + Number(row.target_pct), 0)
  if (out.length && Math.abs(total - 100) > 1e-9) fail(`asset class shares add up to ${total}, not 100`)
  const known = new Set((out.length ? out : PRESET).map((row) => row.id))
  links.splice(0, links.length, ...links.filter((link) => known.has(link.class_id)))
  for (const store of ctx.stores) if (store.asset_class && !known.has(store.asset_class)) store.asset_class = undefined
  saved.splice(0, saved.length, ...out)
  return view()
}

function assign(target: AssetClassTarget, classId: string | null, ctx: PlanningMockContext): AssetClassesView {
  if (classId !== null && !effective().some((row) => row.id === classId)) fail(`asset class ${classId}`, 'MISSING')
  if (!saved.length && classId !== null) saved.push(...PRESET.map((row) => ({ ...row })))
  if (target.kind === 'asset') {
    const kept = links.filter((link) => link.asset_key !== target.asset_key)
    links.splice(0, links.length, ...kept)
    if (classId !== null) links.push({ asset_key: target.asset_key, class_id: classId })
  } else {
    if (classId !== null && fund.scope.store_ids.includes(target.store_id))
      fail('an emergency fund account stays outside the baskets')
    const store = ctx.stores.find((row) => row.id === target.store_id) ?? fail(`store ${target.store_id}`, 'MISSING')
    store.asset_class = classId ?? undefined
  }
  return view()
}

/** Value by basket id (`''` = no basket) and the fund's part. */
export function classValues(ctx: PlanningMockContext) {
  const byClass = new Map<string, number>()
  let emergency = 0
  const stores = new Map(ctx.stores.map((store) => [store.id, store]))
  for (const leg of ctx.transactions.flatMap((tx) => txLegs(tx))) {
    const store = stores.get(leg.store_id)
    if (leg.role !== 'principal' || !store || OUTSIDE.has(store.kind)) continue
    const value = Number(leg.amount) * ctx.priceOf(leg.asset.symbol)
    if (fund.scope.store_ids.includes(store.id)) {
      emergency += value
      continue
    }
    const key = assetKey(leg.asset)
    const slot = links.find((link) => link.asset_key === key)?.class_id ?? store.asset_class ?? ''
    byClass.set(slot, (byClass.get(slot) ?? 0) + value)
  }
  return { byClass, emergency }
}

function split(classes: AssetClass[], values: number[], amount: number): [number, boolean][] {
  const total = values.reduce((sum, value) => sum + value, 0)
  const capped = classes.map(
    (row, index) => row.cap && total > 0 && values[index] / total > Number(row.target_pct) / 100,
  )
  const grown = total + amount
  const deficits = classes.map((row, index) =>
    capped[index] ? 0 : Math.max(0, (grown * Number(row.target_pct)) / 100 - values[index]),
  )
  const sumDeficit = deficits.reduce((sum, value) => sum + value, 0)
  const weights = sumDeficit > 0 ? deficits : classes.map((row, index) => (capped[index] ? 0 : Number(row.target_pct)))
  const sumWeight = weights.reduce((sum, value) => sum + value, 0)
  return weights.map((weight, index) => [sumWeight ? Number(money((amount * weight) / sumWeight)) : 0, capped[index]])
}

function advice(given: string | null, ctx: PlanningMockContext): TopupAdvice {
  const today = now()
  const last = new Date(today.getFullYear(), today.getMonth() - 1, 1)
  const month = `${last.getFullYear()}-${String(last.getMonth() + 1).padStart(2, '0')}`
  const totals = monthTotals(ctx, month)
  const amount = given === null ? Math.max(0, totals.income - totals.fixed - totals.daily) : Math.max(0, Number(given))
  const { byClass, emergency } = classValues(ctx)
  const classes = effective()
  const values = classes.map((row) => byClass.get(row.id) ?? 0)
  const total = values.reduce((sum, value) => sum + value, 0)
  const share = (part: number, whole: number) => (whole ? money((part / whole) * 100) : null)
  const rows = split(classes, values, amount).map(([add, capped], index) => ({
    class: classes[index],
    value: money(values[index]),
    pct_before: share(values[index], total),
    add: money(add),
    pct_after: share(values[index] + add, total + amount),
    capped,
  }))
  return {
    currency: 'usd',
    amount: money(amount),
    from_savings: given === null,
    savings_month: given === null ? month : null,
    rows,
    unclassified: money(byClass.get('') ?? 0),
    emergency: money(emergency),
    preset: !saved.length,
  }
}

export function planAllocation(cmd: string, args: Record<string, unknown> | undefined, ctx: PlanningMockContext) {
  switch (cmd) {
    case 'asset_classes_get':
      return view()
    case 'asset_classes_set':
      return setClasses(args?.classes as AssetClass[], ctx)
    case 'asset_class_assign':
      return assign(args?.target as AssetClassTarget, (args?.classId as string | null | undefined) ?? null, ctx)
    case 'topup_advice':
      return advice((args?.amount as string | null | undefined) ?? null, ctx)
    default:
      return undefined
  }
}
