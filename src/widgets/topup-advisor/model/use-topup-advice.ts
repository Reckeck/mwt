import { computed, ref, watch } from 'vue'
import { refDebounced } from '@vueuse/core'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { TOPUP_ADVICE_KEY } from '@/shared/api/queries'
import { parseDecimal } from '@/shared/lib/decimal'
import { nonNegativeAmount } from '@/shared/lib/form'

const TYPING_PAUSE_MS = 400

/**
 * The advice for the amount typed, or for last month's savings while the field
 * is empty. A typo keeps the last good advice on screen and shows its message.
 */
export function useTopupAdvice() {
  const amountText = ref('')
  const settled = refDebounced(amountText, TYPING_PAUSE_MS)
  const error = computed(() => nonNegativeAmount(amountText.value))
  /** The amount asked for: `null` is «last month's savings». */
  const amount = ref<string | null>(null)
  watch(settled, (text) => {
    if (!nonNegativeAmount(text)) amount.value = text.trim() ? parseDecimal(text) : null
  })

  const advice = useQuery({
    queryKey: computed(() => [...TOPUP_ADVICE_KEY, amount.value]),
    queryFn: () => call<TopupAdvice>('topup_advice', { amount: amount.value }),
    placeholderData: keepPreviousData,
  })

  return { amountText, error, advice }
}
