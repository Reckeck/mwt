import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/** Under `workflows`: a run, an answer or a switch refreshes the journal. */
export const JOURNAL_KEY = [...WORKFLOW_KEYS.list, 'journal']

/** The journal: what ran, the newest first. */
export function useJournal() {
  return useQuery({
    queryKey: JOURNAL_KEY,
    queryFn: () => call<JournalEntry[]>('list_automation_journal', { limit: 200 }),
    placeholderData: keepPreviousData,
  })
}
