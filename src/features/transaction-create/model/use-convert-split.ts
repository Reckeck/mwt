import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { parseDecimal } from '@/shared/lib/decimal'
import { formatQuantity } from '@/shared/lib/format'

import { useTxForm } from './use-tx-form'

/**
 * What an exchange with several destinations shows: whether it is split, the first
 * destination's implied share (the placeholder of its own amount box) and the
 * `Distributed 480 of 480 USDT` line. Shared by the short and the full form.
 */
export function useConvertSplit() {
  const { t } = useI18n()
  const { form, distributedTotal } = useTxForm()

  const split = computed(() => form.extra.length > 0)
  const total = computed(() => Number(parseDecimal(form.toAmount) ?? 0))
  /** The total minus what the extra rows take, or `''` when nothing positive is left to imply. */
  const remainder = computed(() => {
    const extra = form.extra.reduce((sum, row) => sum + Number(parseDecimal(row.receive) ?? 0), 0)
    const left = total.value - extra
    return left > 0 ? formatQuantity(left) : ''
  })
  const distributedLine = computed(() =>
    t('tx.split.distributed', {
      done: formatQuantity(distributedTotal.value),
      total: `${formatQuantity(total.value)} ${form.toAsset?.symbol ?? ''}`.trim(),
    }),
  )
  return { split, remainder, distributedLine }
}
