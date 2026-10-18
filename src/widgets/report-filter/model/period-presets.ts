/** Quick periods of the report filter; months are `YYYY-MM`. */

export type PeriodPreset = 'last12' | 'thisMonth' | 'last3' | 'thisYear' | 'lastYear'

export const PERIOD_PRESETS: PeriodPreset[] = ['last12', 'thisMonth', 'last3', 'thisYear', 'lastYear']

const key = (year: number, month: number) => {
  const date = new Date(year, month, 1, 12)
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`
}

/** `from`/`to` of a preset; `last12` is the reports' default, so both stay empty. */
export function presetRange(preset: PeriodPreset, now = new Date()): { from: string | null; to: string | null } {
  const year = now.getFullYear()
  const month = now.getMonth()
  switch (preset) {
    case 'last12':
      return { from: null, to: null }
    case 'thisMonth':
      return { from: key(year, month), to: key(year, month) }
    case 'last3':
      return { from: key(year, month - 2), to: key(year, month) }
    case 'thisYear':
      return { from: key(year, 0), to: key(year, month) }
    case 'lastYear':
      return { from: key(year - 1, 0), to: key(year - 1, 11) }
  }
}

/** The preset a range matches, or `''` for a custom one. */
export function presetOf(from: string | null, to: string | null, now = new Date()): PeriodPreset | '' {
  return (
    PERIOD_PRESETS.find((preset) => {
      const range = presetRange(preset, now)
      return range.from === from && range.to === to
    }) ?? ''
  )
}

/** What an empty `from`/`to` means: eleven months back, and this month. */
export function defaultRange(now = new Date()): { from: string; to: string } {
  return { from: key(now.getFullYear(), now.getMonth() - 11), to: key(now.getFullYear(), now.getMonth()) }
}
