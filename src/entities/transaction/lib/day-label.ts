import { formatDay } from '@/shared/lib/format'

/** «Ср, 21 окт»: capital first letter only, no abbreviation dots. */
export function dayLabel(stamp: string): string {
  const text = formatDay(stamp).replaceAll('.', '')
  return text.charAt(0).toUpperCase() + text.slice(1)
}
