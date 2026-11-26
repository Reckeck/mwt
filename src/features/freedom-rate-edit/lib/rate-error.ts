import { parseDecimal } from '@/shared/lib/decimal'
import { firstError, nonNegativeAmount } from '@/shared/lib/form'

/** The core takes a yearly return of 0..100 percent (`health_settings_set`); the field says so before it does. */
export const RATE_PCT_MAX = 100

type Translate = (key: string, values: Record<string, unknown>) => string

export const rateError = (draft: string, t: Translate) =>
  firstError(
    nonNegativeAmount(draft),
    Number(parseDecimal(draft) ?? 0) > RATE_PCT_MAX ? t('validation.range', { min: 0, max: RATE_PCT_MAX }) : null,
  )
