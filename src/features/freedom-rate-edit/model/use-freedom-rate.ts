import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { CAPITAL_HEALTH_KEY } from '@/shared/api/queries'
import { parseDecimal } from '@/shared/lib/decimal'
import { firstError, nonNegativeAmount, useFormValidation } from '@/shared/lib/form'
import { textOf } from '@/shared/lib/notify'

/** The core takes a yearly return of 0..100 percent (`health_settings_set`); the field says so before it does. */
const RATE_PCT_MAX = 100

interface RateChange {
  rate: Amount | null
  previous: Amount | null
}

/** The expected return behind the freedom potential; blank = the key rate. */
export function useFreedomRate(current: () => Amount | null) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const draft = ref('')
  watch(current, (rate) => (draft.value = rate ?? ''), { immediate: true })

  const form = useFormValidation(() => ({
    rate: firstError(
      nonNegativeAmount(draft.value),
      Number(parseDecimal(draft.value) ?? 0) > RATE_PCT_MAX
        ? t('validation.range', { min: 0, max: RATE_PCT_MAX })
        : null,
    ),
  }))
  const save = useMutation({
    mutationFn: ({ rate }: RateChange) => call<null>('health_settings_set', { expectedReturn: rate }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: CAPITAL_HEALTH_KEY }),
    meta: {
      error: 'health.freedom.saveFailed',
      success: () => textOf('health.freedom.saved'),
      undo: (_data, change) => call<null>('health_settings_set', { expectedReturn: (change as RateChange).previous }),
    },
  })

  function submit() {
    const rate = draft.value.trim() ? parseDecimal(draft.value) : null
    save.mutate({ rate, previous: current() })
  }

  return { draft, form, save, submit }
}
