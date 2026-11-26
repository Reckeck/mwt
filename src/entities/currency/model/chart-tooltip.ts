import { type Ref, computed } from 'vue'

import { formatMoney } from '@/shared/lib/format'

import { useConversions } from './conversions'

interface TooltipParam {
  marker?: string
  seriesName?: string
  value?: unknown
  data?: unknown
  axisValueLabel?: string
  name?: string
}

/** The number a tooltip param carries: a plain value, or the last of a `[time, value]` pair. */
function pointValue(param: TooltipParam): number {
  const raw = Array.isArray(param.value) ? param.value[param.value.length - 1] : param.value
  const value = Number(raw)
  return Object.is(value, -0) ? 0 : value
}

const escapeHtml = (text: string) =>
  text.replace(/[&<>"]/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[char] ?? char)

/**
 * An ECharts tooltip formatter for money charts: each point in the chart's
 * currency and, under it, the same amount in the base and favourite
 * currencies (rule: every chart shows them all, like the hover over an amount).
 */
export function useMoneyChartTooltip(currency: Ref<string>) {
  const conversions = useConversions(true)
  return computed(() => {
    // Read the table here so the option recomputes once the rates arrive.
    void conversions.table.data.value
    const code = currency.value.toLowerCase()
    return (raw: unknown): string => {
      const params = (Array.isArray(raw) ? raw : [raw]) as TooltipParam[]
      const title = params[0]?.axisValueLabel ?? params[0]?.name ?? ''
      const lines = params.map((param) => {
        const amount = pointValue(param)
        if (!Number.isFinite(amount)) return ''
        const head = `${param.marker ?? ''}${param.seriesName ? `${escapeHtml(param.seriesName)}: ` : ''}<b>${escapeHtml(formatMoney(amount, { currency: code }))}</b>`
        const others = conversions
          .convert(amount, code)
          .map(
            (entry) =>
              `<div style="opacity:.7;padding-left:14px">≈ ${escapeHtml(entry.text)}${entry.stale ? ` · ${escapeHtml(entry.stale)}` : ''}</div>`,
          )
          .join('')
        return `<div>${head}</div>${others}`
      })
      return `<div style="font-weight:500;margin-bottom:2px">${escapeHtml(title)}</div>${lines.join('')}`
    }
  })
}
