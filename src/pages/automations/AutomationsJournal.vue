<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { WorkflowQueue } from '@/widgets/workflow-queue'
import { AutomationConfirmDialog } from '@/features/workflow-manage'
import { useIsPhone } from '@/shared/lib/phone'
import { buttonVariants } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'

import { useJournal } from './model/use-journal'
import { JournalList, JournalTable } from './ui/journal'

/**
 * The journal (Auto-1 · 4): everything that ran — schemes, repeats, reminders,
 * alerts — in one list, the newest first. A run waiting for `Yes` is confirmed
 * from its row (Auto-1 · 5).
 */
const { t } = useI18n()
const phone = useIsPhone()
const { list, rows, queued, confirming, confirmOpen, storeName } = useJournal()
const confirm = (entry: JournalEntry) => (confirming.value = entry)
</script>

<template>
  <SectionState :loading="list.isPending.value" :error="list.error.value" skeleton="list" :rows="5">
    <EmptyState v-if="!rows.length" :title="t('workflow.journal.empty')" :description="t('workflow.journal.emptyHint')">
      <RouterLink :to="{ name: 'automations-workflows' }" :class="buttonVariants({ variant: 'outline' })">
        {{ t('workflow.journal.toLessons') }}
      </RouterLink>
    </EmptyState>
    <div v-else class="grid gap-6">
      <div class="min-w-0" data-tour="automations-journal">
        <JournalList v-if="phone" :rows="rows" @confirm="confirm" />
        <JournalTable v-else :rows="rows" @confirm="confirm" />
      </div>
      <!-- Operations a scheme prepared for confirmation are not runs: they wait under the journal. -->
      <section v-if="queued" class="grid gap-3">
        <h2 class="heading-section">{{ t('workflow.journal.waiting') }}</h2>
        <WorkflowQueue />
      </section>
    </div>
  </SectionState>
  <AutomationConfirmDialog v-model:open="confirmOpen" :entry="confirming" :store-name="storeName" />
</template>
