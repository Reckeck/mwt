/** Steps the shares can be rounded to (the backend takes only these). */
export const ROUNDING_STEPS = ['0.01', '1', '10', '50', '100'] as const

export const DEFAULT_ROUNDING_STEP = '1'

export const EMPTY_DETAILS: GuestDetails = { phone: '', messenger: '', requisites: [], note: '' }
