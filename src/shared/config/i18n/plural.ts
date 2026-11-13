/**
 * Plural forms per locale. vue-i18n's own rule is English-shaped: with three
 * messages it picks `0 -> 0`, `1 -> 1`, `2+ -> 2`, so Russian came out wrong
 * («каждые 1 месяца», «каждые 2 месяцев»).
 */

/**
 * Russian: one (1, 21, 31…), few (2-4, 22-24…), many (0, 5-20, 25-30…).
 * Slot 0 keeps vue-i18n's meaning for `t(key, 0)` — the wording without a
 * number — so a message written for the old rule keeps working.
 */
export function ruPlural(choice: number, choicesLength: number): number {
  const count = Math.abs(choice)
  if (choicesLength < 3) return count === 1 ? 0 : 1
  if (!count) return 0
  const tens = count % 10
  const hundreds = count % 100
  if (tens === 1 && hundreds !== 11) return 0
  if (tens >= 2 && tens <= 4 && (hundreds < 12 || hundreds > 14)) return 1
  return 2
}
