import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { statusTone } from '@/widgets/automation-details'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

import { TONE_CLASS, journalDid, journalWhen } from '../lib/journal-view'

/** Under `workflows`: a run, an answer or a switch refreshes the journal. */
export const JOURNAL_KEY = [...WORKFLOW_KEYS.list, 'journal']

/** The journal (Auto-1 · 4): what ran, the newest first, and the run that waits for «да» to confirm. */
export function useJournal() {
  const { t } = useI18n()
  const list = useQuery({
    queryKey: JOURNAL_KEY,
    queryFn: () => call<JournalEntry[]>('list_automation_journal', { limit: 200 }),
  })
  const queue = useQuery({ queryKey: WORKFLOW_KEYS.queue, queryFn: () => call<QueuedActions[]>('list_workflow_queue') })
  const queued = computed(() => !!queue.data.value?.length)
  const tree = useStoreTree()
  const stores = computed(
    () => new Map(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node.store.name])),
  )
  const names = { store: (id: string) => stores.value.get(id) ?? '', category: (key: string) => key }

  const rows = computed(() =>
    (list.data.value ?? []).map((entry) => ({
      entry,
      when: journalWhen(entry.at),
      did: journalDid(entry, names, t),
      status: t(`automations.status.${entry.status}`),
      tone: TONE_CLASS[statusTone(entry.status)],
    })),
  )

  /** The run the confirm dialog (Auto-1 · 5) is open for. */
  const confirming = ref<JournalEntry | null>(null)
  const confirmOpen = computed({
    get: () => confirming.value !== null,
    set: (value) => {
      if (!value) confirming.value = null
    },
  })

  return { list, rows, queued, confirming, confirmOpen, storeName: names.store }
}
