/** Commands: utility items, meter readings, bills and payments. */

import { money } from './budget-calc'
import { fail, iso, now, readings, remove, replace, utilities, uuid } from './state'
import type { PlanningMockContext } from './state'
import { paymentsOf, utilityTrend } from './utility-calc'
import { meterBill, previewFormula } from './utility-formula'

/** Planning commands: Utility bills: items, meter readings, bills and payments. `undefined` when `cmd` is not one. */
export function planUtilities(
  cmd: string,
  args: Record<string, unknown> | undefined,
  ctx: PlanningMockContext,
  _settings: Settings,
): unknown {
  const body = args?.input as Record<string, unknown> | undefined
  switch (cmd) {
    case 'list_utility_items':
      return utilities
    case 'create_utility_item': {
      const item: UtilityItem = { id: uuid(), ...(body as UtilityInput) }
      utilities.push(item)
      return item
    }
    case 'update_utility_item': {
      const input = body as UpdateUtilityInput
      return replace(utilities, input.id, input.item)
    }
    case 'delete_utility_item':
      remove(utilities, String(args?.id))
      return {}
    case 'add_meter_reading': {
      const input = body as MeterReadingInput
      const reading: MeterReading = {
        id: uuid(),
        ...input,
        day: input.day ?? null,
        zone: input.zone ?? null,
        recorded_at: iso(now()),
      }
      const index = readings.findIndex(
        (entry) => entry.item_id === input.item_id && entry.period === input.period && entry.zone === reading.zone,
      )
      if (index >= 0) readings.splice(index, 1)
      readings.push(reading)
      return reading
    }
    case 'delete_meter_reading':
      remove(readings, String(args?.id))
      return {}
    case 'update_meter_reading': {
      const input = body as UpdateMeterReadingInput
      const reading = readings.find((entry) => entry.id === input.id) ?? fail('not found', 'MISSING')
      Object.assign(reading, { value: input.value, period: input.period, day: input.day ?? null })
      return reading
    }
    case 'get_utility_bill': {
      const propertyId = String(args?.propertyId ?? '')
      const period = String(args?.period ?? iso(now()).slice(0, 7))
      const property = ctx.stores.find((store) => store.id === propertyId)
      const payDay = property?.property?.utilities_pay_day ?? null
      const lines: UtilityBillLine[] = utilities
        .filter((item) => item.active && item.property_id === propertyId)
        .map((item) => {
          const zones = 'metered' in item.kind ? item.kind.metered.zones : []
          const readingsOf = (zone: string | null) => {
            const own = readings
              .filter((entry) => entry.item_id === item.id && (entry.zone ?? null) === zone)
              .sort((left, right) => left.period.localeCompare(right.period))
            return {
              zone,
              previous: own.filter((entry) => entry.period < period).at(-1)?.value ?? null,
              current: own.find((entry) => entry.period === period)?.value ?? null,
            }
          }
          const parts = zones.length ? zones.map(readingsOf) : [readingsOf(null)]
          const sum = (key: 'previous' | 'current') =>
            parts.every((part) => part[key] !== null)
              ? String(parts.reduce((total, part) => total + Number(part[key]), 0))
              : null
          const [previous, current] = [sum('previous'), sum('current')]
          const fixed =
            'static' in item.kind ? item.kind.static.amount : 'housing' in item.kind ? item.kind.housing.amount : null
          const consumption = current !== null && previous !== null ? Number(current) - Number(previous) : null
          const paid = (paymentsOf(item).get(period) ?? []).reduce(
            (sum, tx) => sum + ('payment' in tx ? Number(tx.payment.amount) : 0),
            0,
          )
          return {
            item,
            previous_reading: previous,
            reading: current,
            consumption: consumption === null ? null : String(consumption),
            amount:
              'metered' in item.kind
                ? consumption === null
                  ? null
                  : money(
                      meterBill(
                        item,
                        parts.map((part) => ({
                          zone: part.zone,
                          previous: part.previous ?? '0',
                          current: part.current ?? '0',
                        })),
                      ),
                    )
                : fixed,
            paid: money(paid),
            includes_deposit: false,
            zones: zones.map((zone, index) => ({
              zone,
              previous_reading: parts[index].previous,
              reading: parts[index].current,
            })),
          }
        })
      const day = String(Math.min(payDay ?? now().getDate(), 28)).padStart(2, '0')
      return {
        property_id: propertyId,
        period,
        pay_day: payDay,
        due_at: `${period}-${day}T12:00:00Z`,
        lines,
      } satisfies UtilityBill
    }
    case 'pay_utility_bill': {
      const input = body as PayUtilityBillInput
      const saved: Transaction[] = []
      for (const line of input.lines) {
        const item = utilities.find((entry) => entry.id === line.item_id) ?? fail('not found', 'MISSING')
        const typed = line.zone_readings.length
          ? line.zone_readings
          : line.reading
            ? [{ zone: null, reading: line.reading }]
            : []
        for (const { zone, reading } of typed) {
          remove(
            readings,
            readings.find((entry) => entry.item_id === item.id && entry.period === input.period && entry.zone === zone)
              ?.id ?? '',
          )
          readings.push({
            id: uuid(),
            item_id: item.id,
            value: reading,
            period: input.period,
            recorded_at: iso(now()),
            day: line.reading_day ?? null,
            zone,
          })
        }
        const code = item.currency.toUpperCase()
        const ts = input.timestamp || `${input.period}-10T12:00:00Z`
        const tx = ctx.transactionFrom(
          {
            type: 'payment',
            common: {
              timestamp: ts,
              comment: item.name,
              categories: [item.category],
              flow_id: null,
              contact_id: null,
              price_usd: null,
              fx_override: null,
              price: null,
              price_currency: null,
            },
            store_id: item.store_id || input.store_id,
            asset: { symbol: code, name: code, coingecko_id: null, chain: 'fiat', contract: null },
            amount: line.amount,
            fee: null,
            to: null,
            details: { cash_breakdown: null, unit_price: null, network: null, tx_hash: null },
          } as TransactionInput,
          ts,
        )
        if ('payment' in tx) tx.payment.meta.utility_item_id = item.id
        ctx.transactions.push(tx)
        saved.push(tx)
      }
      return saved
    }
    case 'get_property_summary': {
      const storeId = String(args?.storeId ?? '')
      const own = utilities.filter((item) => item.active && item.property_id === storeId)
      const monthly = own.reduce((sum, item) => sum + Number(utilityTrend(item).monthly), 0)
      const summary: PropertySummary = {
        store_id: storeId,
        base_currency: 'usd',
        value_base: '0',
        debt_base: '0',
        equity_base: '0',
        utilities_month_base: String(monthly),
        loan_month_base: '0',
        monthly_cost_base: String(monthly),
        utility_count: own.length,
      }
      return summary
    }
    case 'preview_utility_formula':
      return previewFormula(body as UtilityFormulaInput)
    case 'get_meter_trend': {
      const item = utilities.find((entry) => entry.id === args?.itemId) ?? fail('not found', 'MISSING')
      return utilityTrend(item)
    }
    case 'get_utility_payment_draft': {
      const item = utilities.find((entry) => entry.id === args?.itemId) ?? fail('not found', 'MISSING')
      const trend = utilityTrend(item)
      const code = item.currency.toUpperCase()
      const tx = ctx.transactionFrom(
        {
          type: 'payment',
          common: {
            timestamp: null,
            comment: item.name,
            categories: [item.category],
            flow_id: null,
            contact_id: null,
            price_usd: null,
            fx_override: null,
            price: null,
            price_currency: null,
          },
          store_id: item.store_id ?? '',
          asset: { symbol: code, name: code, coingecko_id: null, chain: 'fiat', contract: null },
          amount: trend.monthly,
          fee: null,
          to: null,
          details: { cash_breakdown: null, unit_price: null, network: null, tx_hash: null },
        } as TransactionInput,
        iso(now()),
      )
      return {
        item_id: item.id,
        period: String(args?.period ?? iso(now()).slice(0, 7)),
        transaction: tx,
        consumption: null,
        includes_deposit: false,
      } satisfies UtilityPaymentDraft
    }
    default:
      return undefined
  }
}
