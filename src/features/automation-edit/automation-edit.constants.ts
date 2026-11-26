/** Schedule kinds the dialog offers, in display order. */
export const SCHEDULE_KINDS = ['monthly', 'weekly', 'every_n_days'] as const
export type ScheduleKind = (typeof SCHEDULE_KINDS)[number]

/** Where a date on a weekend or a holiday goes, in display order. */
export const WORKDAY_SHIFTS: readonly WorkdayShift[] = ['none', 'back', 'forward']

/** Step 1 of the wizard (Auto-1 · 6): what should happen, in the frame's order. */
export const WIZARD_TYPES = ['repeat', 'reminder', 'alert', 'scheme'] as const
export type WizardType = (typeof WIZARD_TYPES)[number]

/** `Or start from a ready one`: the first two the wizard fills itself, the others open their own form. */
export const WIZARD_STARTERS = ['save_share', 'low_balance', 'weekly_buy', 'workday_payment'] as const
export type WizardStarter = (typeof WIZARD_STARTERS)[number]
/** The two a phone shows (Auto-1 · 6, the sheet). */
export const WIZARD_PHONE_STARTERS: readonly WizardStarter[] = ['save_share', 'low_balance']

/** Step 2 (Auto-1 · 7): what it does. */
export const WIZARD_ACTIONS = ['operation', 'remind', 'share'] as const
export type WizardAction = (typeof WIZARD_ACTIONS)[number]

/** Step 3 (Auto-1 · 8): `How often`. */
export const WIZARD_FREQUENCIES = ['monthly', 'weekly', 'daily'] as const
export type WizardFrequency = (typeof WIZARD_FREQUENCIES)[number]

/** When a repeat fires unless told otherwise. */
export const WIZARD_DEFAULT_TIME = '09:00'
