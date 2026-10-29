/**
 * `When` of a reminder as one choice (Auto-2 · 10): `Every 10th` reads
 * better than `every 1 month` next to a date. The day, the weekday and the
 * date come from the reminder's first date; anything else is `Other interval`.
 */
export type WhenPreset = 'once' | RepeatUnit | 'custom'

export const WHEN_PRESETS: WhenPreset[] = ['once', 'day', 'week', 'month', 'year', 'custom']

/** The preset a saved repeat reads as: every 2 weeks is a custom interval. */
export function presetOf(repeat: ReminderRepeat | null | undefined): WhenPreset {
  if (!repeat) return 'once'
  return repeat.every === 1 ? repeat.unit : 'custom'
}

/** The repeat a preset saves; a custom one takes its own count and unit. */
export function repeatOf(preset: WhenPreset, custom: ReminderRepeat): ReminderRepeat | null {
  if (preset === 'once') return null
  if (preset === 'custom')
    return { every: Math.max(1, Math.min(999, Math.round(custom.every) || 1)), unit: custom.unit }
  return { every: 1, unit: preset }
}
