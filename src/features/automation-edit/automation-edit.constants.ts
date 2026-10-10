/** Schedule kinds the dialog offers, in display order. */
export const SCHEDULE_KINDS = ['monthly', 'weekly', 'every_n_days'] as const
export type ScheduleKind = (typeof SCHEDULE_KINDS)[number]
