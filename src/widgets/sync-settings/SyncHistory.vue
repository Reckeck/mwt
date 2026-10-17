<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Undo2 } from 'lucide-vue-next'

import { SYNC_HISTORY_KEY, SyncChanges, useSyncActions } from '@/features/vault-sync'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { SectionState } from '@/shared/ui/section-state'
import { SettingsSection } from '@/shared/ui/settings-section'
import { Spinner } from '@/shared/ui/spinner'

/** The batches taken from other devices (20, a week): what each changed, «Отменить». */
const { t } = useI18n()
const { undo } = useSyncActions()
const history = useQuery({ queryKey: SYNC_HISTORY_KEY, queryFn: () => call<AppliedBatch[]>('sync_history') })
const list = computed(() => history.data.value ?? [])
</script>

<template>
  <SettingsSection :title="t('sync.history.title')" :description="t('sync.history.description')">
    <SectionState
      :loading="history.isPending.value"
      :error="history.error.value"
      :empty="!list.length"
      :empty-title="t('sync.history.empty')"
      :empty-description="t('sync.history.emptyHint')"
      skeleton="list"
      compact
      @retry="history.refetch()"
    >
      <ul class="grid gap-2">
        <li v-for="batch in list" :key="batch.id">
          <Collapsible class="rounded-md border border-border">
            <div class="flex items-center gap-2 px-3 py-2">
              <CollapsibleTrigger class="grid min-w-0 flex-1 gap-0.5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                <span class="truncate text-sm font-medium">
                  {{ t('sync.history.from', { name: batch.peer_name, count: batch.summary.total }) }}
                </span>
                <span class="text-xs text-muted-foreground">{{ formatDateTime(batch.applied_at) }}</span>
              </CollapsibleTrigger>
              <Badge v-if="batch.undone_at" variant="outline">{{ t('sync.history.undone') }}</Badge>
              <Button
                v-else
                size="sm"
                variant="outline"
                class="gap-1.5"
                :disabled="undo.isPending.value"
                @click="undo.mutate(batch.id)"
              >
                <Spinner v-if="undo.isPending.value && undo.variables.value === batch.id" />
                <Undo2 v-else class="size-3.5" />
                {{ t('actions.undo') }}
              </Button>
            </div>
            <CollapsibleContent class="border-t border-border p-3">
              <SyncChanges :summary="batch.summary" />
            </CollapsibleContent>
          </Collapsible>
        </li>
      </ul>
      <p class="mt-2 text-xs text-muted-foreground">{{ t('sync.history.undoHint') }}</p>
    </SectionState>
  </SettingsSection>
</template>
