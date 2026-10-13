<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { AlertTriangle, RefreshCw } from 'lucide-vue-next'

import { useExchangeSync } from '@/features/pending-imports'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
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
    <div v-if="empty" class="rounded-xl border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
      {{ t('syncHistory.empty') }}
    </div>
    <div v-else class="overflow-x-auto rounded-md border border-border">
      <table class="w-full text-sm">
        <thead class="bg-muted text-xs text-muted-foreground">
          <tr>
            <th class="px-2 py-1.5 text-left font-medium">{{ t('syncHistory.when') }}</th>
            <th class="px-2 py-1.5 text-left font-medium">{{ t('syncHistory.source') }}</th>
            <th class="px-2 py-1.5 text-right font-medium">{{ t('syncHistory.fetched') }}</th>
            <th class="px-2 py-1.5 text-right font-medium">{{ t('syncHistory.staged') }}</th>
            <th class="px-2 py-1.5 text-right font-medium">{{ t('syncHistory.known') }}</th>
            <th class="px-2 py-1.5 text-right font-medium">{{ t('syncHistory.filtered') }}</th>
            <th class="px-2 py-1.5 text-left font-medium">{{ t('syncHistory.result') }}</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr v-for="run in runs.data.value ?? []" :key="run.id">
            <td class="whitespace-nowrap px-2 py-1.5 tabular-nums">{{ formatDateTime(run.at) }}</td>
            <td class="px-2 py-1.5">{{ t(`syncHistory.sourceKind.${run.source}`) }}</td>
            <template v-if="run.error">
              <td colspan="5" class="px-2 py-1.5 text-destructive">
                <span class="flex items-start gap-1.5">
                  <AlertTriangle class="mt-0.5 size-4 shrink-0" />
                  <span>{{ t(`import.notice.${run.error}`) }} — {{ t(`syncHistory.fix.${run.error}`) }}</span>
                </span>
              </td>
            </template>
            <template v-else>
              <td class="px-2 py-1.5 text-right tabular-nums">{{ run.fetched }}</td>
              <td class="px-2 py-1.5 text-right tabular-nums">{{ run.staged }}</td>
              <td class="px-2 py-1.5 text-right tabular-nums text-muted-foreground">{{ run.known }}</td>
              <td class="px-2 py-1.5 text-right tabular-nums text-muted-foreground">{{ run.filtered }}</td>
              <td class="px-2 py-1.5">
                <span class="flex flex-wrap gap-1">
                  <Badge v-if="run.confirmed" variant="secondary">{{ t('syncHistory.confirmed', { count: run.confirmed }) }}</Badge>
                  <Badge v-else-if="run.staged" variant="outline">{{ t('syncHistory.waiting') }}</Badge>
                  <Badge v-if="run.reconcile_tx_id" variant="outline">{{ t('syncHistory.reconciled') }}</Badge>
                  <span v-if="!run.staged && !run.reconcile_tx_id" class="text-xs text-muted-foreground">{{ t('syncHistory.nothingNew') }}</span>
                </span>
              </td>
            </template>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
