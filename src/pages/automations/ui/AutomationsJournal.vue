<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Bell, CalendarClock, Repeat, Workflow } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { AutomationConfirmDialog } from '@/features/workflow-manage'
import { WorkflowQueue } from '@/widgets/workflow-queue'
import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'
import { useQuery } from '@tanstack/vue-query'
import { formatDayMonthShort } from '@/shared/lib/format'
import { Card } from '@/shared/ui/card'
import { ListRow } from '@/shared/ui/list-row'
import { SectionState } from '@/shared/ui/section-state'

import { journalTone } from '../lib/journal-view'
import { useJournal } from '../model/use-journal'

/** Everything that ran — schemes, repeats, reminders, alerts — in one list, the newest first. */
const { t } = useI18n()
const journal = useJournal()
const queue = useQuery({ queryKey: WORKFLOW_KEYS.queue, queryFn: () => call<QueuedActions[]>('list_workflow_queue') })
const tree = useStoreTree()
const stores = computed(() => new Map(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node.store.name])))
const storeName = (id: string) => stores.value.get(id) ?? ''
/** The run the confirm dialog is open for: a run waiting for `Yes`. */
const confirming = ref<JournalEntry | null>(null)
const confirmOpen = computed({
  get: () => confirming.value !== null,
  set: (value: boolean) => {
    if (!value) confirming.value = null
  },
})
const KIND_ICON = { repeat: Repeat, scheme: Workflow, reminder: CalendarClock, alert: Bell } as const
</script>

<template>
  <SectionState
    :loading="journal.isPending.value"
    :error="journal.error.value"
    :empty="!journal.data.value?.length"
    :empty-title="t('workflow.journal.empty')"
    :empty-description="t('workflow.journal.emptyHint')"
    :rows="5"
    @retry="journal.refetch()"
  >
    <Card size="sm" data-tour="automations-journal">
      <ul class="divide-y divide-border">
        <li v-for="entry in journal.data.value" :key="entry.id" class="flex items-center gap-3" :data-journal-row="entry.status">
          <div class="min-w-0 flex-1">
            <ListRow
              :title="entry.name"
              :subtitle="`${formatDayMonthShort(entry.at, ', ')}${entry.note ? ` · ${entry.note}` : ''}`"
              :icon="KIND_ICON[entry.kind]"
            >
              <template v-if="entry.money.amount && entry.money.currency" #value>
                <Money :amount="entry.money.amount" :currency="entry.money.currency" />
              </template>
            </ListRow>
          </div>
          <Button
            v-if="entry.wait_id"
            size="sm"
            class="shrink-0"
            :aria-label="t('automations.journal.confirmName', { name: entry.name })"
            data-journal-confirm
            data-tour="journal-confirm"
            @click="confirming = entry"
          >
            {{ t('automations.journal.confirm') }}
          </Button>
          <span v-else class="shrink-0 text-sm" :class="journalTone(entry.status)">{{ t(`automations.status.${entry.status}`) }}</span>
        </li>
      </ul>
    </Card>
    <!-- Operations a scheme prepared for confirmation are not runs: they wait under the journal. -->
    <section v-if="queue.data.value?.length" class="mt-4 grid gap-3">
      <h2 class="heading-section">{{ t('workflow.journal.waiting') }}</h2>
      <WorkflowQueue />
    </section>
  </SectionState>
  <AutomationConfirmDialog v-model:open="confirmOpen" :entry="confirming" :store-name="storeName" />
</template>
