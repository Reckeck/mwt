<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useSyncActions, useSyncJournal } from '@/features/vault-sync'
import { formatDateTime } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import SyncPendingChanges from './SyncPendingChanges.vue'

/**
 * «История»: the changes waiting for an answer, then the journal of what came from
 * (or went to) the other devices: when, which device, what, how it ended. A phone gets a list.
 */
const { t } = useI18n()
const { journal, rows } = useSyncJournal()
const { undo } = useSyncActions()
const isPhone = useIsPhone()
const empty = computed(() => journal.isSuccess.value && !rows.value.length)
</script>

<template>
  <div class="grid gap-4" data-section="sync-history">
    <SyncPendingChanges />
    <Card data-tour="sync-history">
      <SectionState
        :loading="journal.isPending.value"
        :error="journal.error.value"
        :empty="empty"
        :empty-title="t('sync.history.empty')"
        :empty-description="t('sync.history.emptyHint')"
        skeleton="list"
        compact
        @retry="journal.refetch()"
      >
        <ul v-if="isPhone" class="grid">
          <li v-for="row in rows" :key="row.id" class="grid gap-1 border-b border-border py-3 first:pt-0 last:border-b-0 last:pb-0">
            <div class="flex items-baseline justify-between gap-3">
              <span class="min-w-0 truncate text-sm font-medium">{{ row.device }}</span>
              <span class="shrink-0 text-xs" :class="row.tone">{{ row.outcome }}</span>
            </div>
            <p class="text-sm">{{ row.what }}</p>
            <div class="flex items-center justify-between gap-3">
              <span class="text-xs tabular-nums text-muted-foreground">{{ formatDateTime(row.at) }}</span>
              <Button
                v-if="row.undoId"
                size="sm"
                variant="ghost"
               
                :disabled="undo.isPending.value"
                @click="undo.mutate(row.undoId ?? '')"
              >
                {{ t('actions.undo') }}
              </Button>
            </div>
          </li>
        </ul>
        <Table v-else>
          <TableHeader>
            <TableRow class="hover:bg-transparent">
              <TableHead>{{ t('sync.journal.when') }}</TableHead>
              <TableHead>{{ t('sync.journal.device') }}</TableHead>
              <TableHead>{{ t('sync.journal.what') }}</TableHead>
              <TableHead class="text-right">{{ t('sync.journal.result') }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="row in rows" :key="row.id">
              <TableCell class="whitespace-nowrap text-sm tabular-nums text-muted-foreground">{{ formatDateTime(row.at) }}</TableCell>
              <TableCell>{{ row.device }}</TableCell>
              <TableCell class="whitespace-normal">{{ row.what }}</TableCell>
              <TableCell class="text-right text-sm">
                <span class="inline-flex items-center justify-end gap-2">
                  <Button
                    v-if="row.undoId"
                    size="sm"
                    variant="ghost"
                    :disabled="undo.isPending.value"
                    @click="undo.mutate(row.undoId ?? '')"
                  >
                    {{ t('actions.undo') }}
                  </Button>
                  <span :class="row.tone">{{ row.outcome }}</span>
                </span>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
        <p v-if="!empty" class="mt-3 text-xs text-muted-foreground">{{ t('sync.history.undoHint') }}</p>
      </SectionState>
    </Card>
  </div>
</template>
