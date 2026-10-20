/** Schedule kinds the dialog offers, in display order. */
export const SCHEDULE_KINDS = ['monthly', 'weekly', 'every_n_days'] as const
export type ScheduleKind = (typeof SCHEDULE_KINDS)[number]

/** Where a date on a weekend or a holiday goes, in display order. */
export const WORKDAY_SHIFTS: readonly WorkdayShift[] = ['none', 'back', 'forward']
