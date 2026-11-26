import { firstError, integerInRange, required } from '@/shared/lib/form'

import { PAY_DAY_MAX } from '../model/utility-reminders.constants'

/** The utilities pay day: required, a whole day of the month. */
export const payDayError = (day: string) => firstError(required(day), integerInRange(day, 1, PAY_DAY_MAX))
