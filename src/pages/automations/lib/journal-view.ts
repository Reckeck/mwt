import { type Names, type RunTone, cardSum, runMoney } from '@/widgets/automation-details'
import { formatDayMonthShort } from '@/shared/lib/format'
import { composeSums, type SumPart } from '@/shared/lib/money-parts'

type Translate = (key: string, values?: Record<string, unknown>) => string

/** «25 сен, 10:00». */
export function journalWhen(iso: string): string {
  return formatDayMonthShort(iso, ', ')
}

/** «+1 575 $ на Тинькофф», «157,50 $ → Наличные», «уведомление». */
export function journalDid(entry: JournalEntry, names: Names, t: Translate): SumPart[] {
  if (entry.status === 'shown') return [{ text: t('automations.journal.notice') }]
  const { money } = entry
  if (money.amount && money.currency && money.to_store && !money.from_store) {
    const { mark, parts } = composeSums()
    return parts(
      t('automations.journal.income', {
        sum: mark(cardSum(money.amount, money.currency, entry.at)),
        to: names.store(money.to_store),
      }),
    )
  }
  const did = runMoney(entry, names)
  return did.length ? did : [{ text: '—' }]
}

export const TONE_CLASS: Record<RunTone, string> = {
  pos: 'text-success',
  wrn: 'text-warning',
  neg: 'text-destructive',
  mut: 'text-muted-foreground',
}
