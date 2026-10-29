<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Minus, PenLine, Plus } from 'lucide-vue-next'

import { collectionKey, sortedChanges, useSyncActions, useSyncStatus } from '@/features/vault-sync'
import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'

/**
 * `Awaiting confirmation · N`: changes from another device that wait for an answer
 * (the same batch the dialog asks about elsewhere). `Accept` applies them, `Decline` closes the session.
 */
const { t } = useI18n()
const { sessions } = useSyncStatus()
const { decidePlan } = useSyncActions()
const SHOWN = 6

const waiting = computed(() => sessions.value.filter((session) => session.phase === 'awaiting_confirm' && session.plan))
const total = computed(() => waiting.value.reduce((sum, session) => sum + (session.plan?.summary.total ?? 0), 0))
const icons = { added: Plus, updated: PenLine, deleted: Minus } as const

function rows(summary: SyncSummary) {
  return sortedChanges(summary).flatMap((group) =>
    group.items.map((item) => ({ ...item, collection: group.collection })),
  )
}
</script>

<template>
  <SettingsSection
    v-for="session in waiting"
    :key="session.session_id"
    :title="t('sync.pending.title', { count: total })"
    :description="t('sync.pending.from', { name: session.name })"
    data-section="sync-pending"
    data-tour="sync-pending"
  >
    <ul v-if="session.plan" class="grid">
      <li
        v-for="(row, index) in rows(session.plan.summary).slice(0, SHOWN)"
        :key="index"
        class="flex items-center gap-2 border-b border-border py-2 text-sm last:border-b-0"
      >
        <component :is="icons[row.kind]" class="size-3.5 shrink-0 text-muted-foreground" :aria-label="t(`sync.kind.${row.kind}`)" />
        <span class="min-w-0 flex-1 truncate">{{ row.label || t('sync.noName') }}</span>
        <span class="shrink-0 text-xs text-muted-foreground">{{ t(collectionKey(row.collection)) }}</span>
      </li>
      <li v-if="session.plan.summary.total > SHOWN" class="pt-2 text-xs text-muted-foreground">
        {{ t('sync.plan.more', { count: session.plan.summary.total - SHOWN }) }}
      </li>
    </ul>
    <div class="flex justify-end gap-2">
      <Button
        variant="outline"
       
        :disabled="decidePlan.isPending.value"
        @click="decidePlan.mutate({ session_id: session.session_id, accept: false })"
      >
        {{ t('sync.pending.decline') }}
      </Button>
      <Button
       
        :disabled="decidePlan.isPending.value"
        :loading="decidePlan.isPending.value"
        @click="decidePlan.mutate({ session_id: session.session_id, accept: true })"
      >
        {{ t('sync.pending.accept') }}
      </Button>
    </div>
  </SettingsSection>
</template>
