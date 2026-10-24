import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { pluralForm } from '@/shared/lib/format'

import { collectionKey } from '../lib/collections'
import { SYNC_HISTORY_KEY, SYNC_JOURNAL_KEY } from './use-sync-status'

/** The kept batches that can still be undone, by id (the journal line only points at one). */
function undoable(batches: AppliedBatch[] | undefined): Set<string> {
  return new Set((batches ?? []).filter((batch) => !batch.undone_at && !batch.transferred_to).map((batch) => batch.id))
}

/**
 * The sync journal (ring of 200 lines, newest first) for the «История» table:
 * «Что» as a human line («2 операции, 1 лимит») and «Итог» (принято, отклонено, ошибка…).
 */
export function useSyncJournal() {
  const { t, locale } = useI18n()
  const journal = useQuery({ queryKey: SYNC_JOURNAL_KEY, queryFn: () => call<SyncJournalEntry[]>('sync_journal') })
  const batches = useQuery({ queryKey: SYNC_HISTORY_KEY, queryFn: () => call<AppliedBatch[]>('sync_history') })
  const kept = computed(() => undoable(batches.data.value))
  const entries = computed(() => journal.data.value ?? [])

  /** The batch to undo for an applied line, if it is still kept. */
  const undoId = (entry: SyncJournalEntry): string | null =>
    entry.outcome === 'applied' && entry.batch_id && kept.value.has(entry.batch_id) ? entry.batch_id : null

  const tone = (entry: SyncJournalEntry): string => {
    if (entry.outcome === 'failed') return 'text-destructive'
    return entry.outcome === 'applied' ? 'text-success' : 'text-muted-foreground'
  }

  const what = (entry: SyncJournalEntry): string => {
    if (entry.outcome === 'sent' && !entry.counts.length) return t('sync.journal.transfer')
    const [only] = entry.counts
    if (entry.label && entry.counts.length === 1 && only?.count === 1) {
      return t(`sync.journal.item.${collectionKey(only.collection).slice('sync.coll.'.length)}`, { label: entry.label })
    }
    if (!entry.counts.length) return t('sync.plan.nothing')
    return entry.counts
      .map(({ collection, count }) => {
        const name = collectionKey(collection).slice('sync.coll.'.length)
        return t(`sync.journal.count.${name}.${pluralForm(count, locale.value)}`, { count })
      })
      .join(', ')
  }

  const outcome = (entry: SyncJournalEntry): string => {
    const undone = entry.outcome === 'applied' && entry.batch_id && !kept.value.has(entry.batch_id)
    // A line whose batch is gone from the kept ones was either undone or aged out: only the first is known.
    const wasUndone =
      undone && (batches.data.value ?? []).some((batch) => batch.id === entry.batch_id && batch.undone_at)
    return t(`sync.journal.outcome.${wasUndone ? 'undone' : entry.outcome}`)
  }

  /** One model per line, so a template reads fields instead of calling per row. */
  const rows = computed(() =>
    entries.value.map((entry) => ({
      id: entry.id,
      at: entry.at,
      device: entry.peer_name,
      what: what(entry),
      outcome: outcome(entry),
      undoId: undoId(entry),
      tone: tone(entry),
    })),
  )

  return { journal, rows }
}
