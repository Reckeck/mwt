<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { AlertTriangle, RefreshCw } from 'lucide-vue-next'

import { useExchangeSync } from '@/features/pending-imports'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'

/**
 * Every sync of one exchange wallet: when, what started it, how many records
 * came and what became of them, the correction that set the live balance, or
 * why it failed and what to do about it.
 */
const props = defineProps<{ storeId: string }>()
const { t } = useI18n()
const sync = useExchangeSync()

const runs = useQuery({
  queryKey: computed(() => ['import-runs', props.storeId]),
  queryFn: () => call<ImportRun[]>('list_import_runs', { storeId: props.storeId }),
})
const empty = computed(() => runs.isSuccess.value && !(runs.data.value ?? []).length)
</script>

<template>
  <div class="grid gap-3" data-tour="exchange-sync-history">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('syncHistory.intro') }}</p>
      <Button size="sm" variant="outline" :disabled="sync.syncNow.isPending.value" @click="sync.syncNow.mutate(props.storeId)">
        <RefreshCw class="size-4" :class="sync.syncNow.isPending.value && 'animate-spin'" />
        {{ t('import.syncNow') }}
      </Button>
    </div>
    <EmptyState v-if="empty" compact :title="t('syncHistory.empty')" />
    <!-- An error spans the count columns, so this is the shadcn Table, not DataTable. -->
    <div v-else class="overflow-x-auto rounded-md border border-border">
      <Table>
        <TableHeader>
          <TableRow class="hover:bg-transparent">
            <TableHead>{{ t('syncHistory.when') }}</TableHead>
            <TableHead>{{ t('syncHistory.source') }}</TableHead>
            <TableHead class="text-right">{{ t('syncHistory.fetched') }}</TableHead>
            <TableHead class="text-right">{{ t('syncHistory.staged') }}</TableHead>
            <TableHead class="text-right">{{ t('syncHistory.known') }}</TableHead>
            <TableHead class="text-right">{{ t('syncHistory.filtered') }}</TableHead>
            <TableHead>{{ t('syncHistory.result') }}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="run in runs.data.value ?? []" :key="run.id">
            <TableCell class="tabular-nums">{{ formatDateTime(run.at) }}</TableCell>
            <TableCell>{{ t(`syncHistory.sourceKind.${run.source}`) }}</TableCell>
            <TableCell v-if="run.error" colspan="5" class="whitespace-normal text-destructive">
              <span class="flex items-start gap-1.5">
                <AlertTriangle class="mt-0.5 size-4 shrink-0" />
                <span>{{ t(`import.notice.${run.error}`) }} — {{ t(`syncHistory.fix.${run.error}`) }}</span>
              </span>
            </TableCell>
            <template v-else>
              <TableCell class="text-right tabular-nums">{{ run.fetched }}</TableCell>
              <TableCell class="text-right tabular-nums">{{ run.staged }}</TableCell>
              <TableCell class="text-right tabular-nums text-muted-foreground">{{ run.known }}</TableCell>
              <TableCell class="text-right tabular-nums text-muted-foreground">{{ run.filtered }}</TableCell>
              <TableCell>
                <span class="flex flex-wrap gap-1">
                  <Badge v-if="run.confirmed" variant="secondary">{{ t('syncHistory.confirmed', { count: run.confirmed }) }}</Badge>
                  <Badge v-else-if="run.staged" variant="outline">{{ t('syncHistory.waiting') }}</Badge>
                  <Badge v-if="run.reconcile_tx_id" variant="outline">{{ t('syncHistory.reconciled') }}</Badge>
                  <span v-if="!run.staged && !run.reconcile_tx_id" class="text-xs text-muted-foreground">{{ t('syncHistory.nothingNew') }}</span>
                </span>
              </TableCell>
            </template>
          </TableRow>
        </TableBody>
      </Table>
    </div>
  </div>
</template>
