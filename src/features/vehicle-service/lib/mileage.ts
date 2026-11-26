import { integerInRange } from '@/shared/lib/form'

import { KM_MAX } from '../vehicle-service.constants'

/** An odometer or service mileage in km: blank passes, else a whole number up to `KM_MAX`. */
export const kmError = (km: string) => integerInRange(km, 0, KM_MAX)
