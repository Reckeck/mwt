/** Categories in the order the settings list them. */
export const NOTICE_CATEGORIES: NoticeCategory[] = [
  'reminder',
  'loan',
  'rent',
  'utilities',
  'debt',
  'deposit',
  'loyalty',
  'goal',
  'tax',
  'shortfall',
  'budget',
  'price_alert',
  'automation',
  'bybit_import',
]

/** Categories automatic reminders are made for. */
export const AUTO_REMINDER_CATEGORIES: NoticeCategory[] = [
  'loan',
  'rent',
  'utilities',
  'debt',
  'deposit',
  'loyalty',
  'goal',
  'tax',
]

/** Categories whose notice can come ahead of a date. */
export const LEAD_CATEGORIES: NoticeCategory[] = [...AUTO_REMINDER_CATEGORIES, 'shortfall']

/** Defaults of a category nobody set yet; mirrors `NoticeCategory::default_lead_days`. */
export const DEFAULT_LEAD_DAYS: Partial<Record<NoticeCategory, number>> = {
  loan: 1,
  rent: 1,
  utilities: 1,
  debt: 1,
  shortfall: 1,
  deposit: 7,
  loyalty: 7,
  tax: 14,
  goal: 14,
}
