import { type Names, type RunTone, cardSum, runMoney } from '@/widgets/automation-details'
import { TIME_CYCLE, dateFormat, formatDayMonthShort } from '@/shared/lib/format'

type Translate = (key: string, values?: Record<string, unknown>) => string

/** «25 сен, 10:00». */
export function journalWhen(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return `${formatDayMonthShort(iso)}, ${dateFormat({ hour: '2-digit', minute: '2-digit', hourCycle: TIME_CYCLE }).format(date)}`
}

/** «+1 575 $ на Тинькофф», «157,50 $ → Наличные», «уведомление». */
export function journalDid(entry: JournalEntry, names: Names, t: Translate): string {
  if (entry.status === 'shown') return t('automations.journal.notice')
  const { money } = entry
  if (money.amount && money.currency && money.to_store && !money.from_store) {
    return t('automations.journal.income', {
      sum: cardSum(money.amount, money.currency),
      to: names.store(money.to_store),
    })
  }
  return runMoney(entry, names) || '—'
}

export const TONE_CLASS: Record<RunTone, string> = {
  pos: 'text-success',
  wrn: 'text-warning',
  neg: 'text-destructive',
  mut: 'text-muted-foreground',
}
