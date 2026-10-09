<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { Check, Download, X } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { call, parseError } from '@/shared/api/ipc'

import type { PendingImportsProps } from './pending-imports.types'

const props = defineProps<PendingImportsProps>()

const { t } = useI18n()
const queryClient = useQueryClient()

// -- D2.4: Bybit import staging -----------------------------------------
// Fills are staged in the vault and only become transactions when the user
// confirms them, so an import can never silently rewrite history.
const importBusy = ref(false)

const pending = useQuery({
  queryKey: ['pending-imports'],
  queryFn: () => call<ListPendingImportsOutput>('list_pending_imports'),
})

const pendingCount = computed(() => pending.data.value?.imports.length ?? 0)

async function loadHistory() {
  importBusy.value = true
  try {
    const res = await call<ImportBybitHistoryOutput>('import_bybit_history', {
      input: { store_id: props.storeId },
    })
    if (res.notice) {
      toast.warning(t(`import.notice.${res.notice}`))
    } else {
      toast.success(t('import.staged'), { description: String(res.staged) })
    }
    await pending.refetch()
  } catch {
    toast.error(t('import.notice.unreachable'))
  } finally {
    importBusy.value = false
  }
}

async function confirmImports(execIds: string[]) {
  importBusy.value = true
  try {
    const res = await call<ConfirmPendingImportsOutput>('confirm_pending_imports', { input: { exec_ids: execIds } })
    toast.success(t('import.confirmed'), { description: String(res.confirmed) })
    await pending.refetch()
    // Confirmed fills became trades, so every derived view is stale.
    const staleKeys: ReadonlyArray<readonly unknown[]> = [
      ['transactions'],
      ['store', props.storeId],
      ['store-pnl', props.storeId],
      ['summary'],
      ['history'],
    ]
    await Promise.all(
      staleKeys.map((queryKey) => queryClient.invalidateQueries({ queryKey })),
    )
  } catch (err) {
    toast.error(t('import.confirmFailed'), { description: parseError(err).message })
  } finally {
    importBusy.value = false
  }
}

async function discardImports(execIds: string[]) {
  importBusy.value = true
  try {
    const res = await call<DiscardPendingImportsOutput>('discard_pending_imports', { input: { exec_ids: execIds } })
    toast.success(t('import.discarded'), { description: String(res.discarded) })
    await pending.refetch()
  } catch (err) {
    toast.error(t('import.discardFailed'), { description: parseError(err).message })
  } finally {
    importBusy.value = false
  }
}
</script>

<template>
  <!-- D2.4: import history into a staging area, confirmed explicitly -->
  <div class="grid gap-3 border-t border-border pt-3">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm font-medium">
        {{ t('import.title') }}
        <Badge v-if="pendingCount" variant="secondary">{{ pendingCount }}</Badge>
      </p>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" :disabled="importBusy" @click="loadHistory">
          <Download class="size-4" />
          {{ t('import.load') }}
        </Button>
        <Button
          v-if="pendingCount"
          size="sm"
          :disabled="importBusy"
          @click="confirmImports([])"
        >
          <Check class="size-4" />
          {{ t('import.confirmAll') }}
        </Button>
      </div>
    </div>
    <p class="text-xs text-muted-foreground">{{ t('import.hint') }}</p>

    <div v-if="pendingCount" class="max-h-72 overflow-auto rounded-md border border-border">
      <table class="w-full text-sm">
        <thead class="sticky top-0 bg-muted">
          <tr>
            <th class="px-2 py-1.5 text-left font-medium">{{ t('import.date') }}</th>
            <th class="px-2 py-1.5 text-left font-medium">{{ t('import.pair') }}</th>
            <th class="px-2 py-1.5 text-left font-medium">{{ t('import.side') }}</th>
            <th class="px-2 py-1.5 text-right font-medium">{{ t('import.qty') }}</th>
            <th class="px-2 py-1.5 text-right font-medium">{{ t('import.price') }}</th>
            <th class="px-2 py-1.5" />
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr v-for="imp in pending.data.value?.imports ?? []" :key="imp.exec_id">
            <td class="px-2 py-1.5 font-mono text-xs">{{ imp.occurred_at.slice(0, 10) }}</td>
            <td class="px-2 py-1.5">{{ imp.symbol }}</td>
            <td class="px-2 py-1.5">
              <Badge :variant="imp.side === 'buy' ? 'default' : 'secondary'">
                {{ imp.side === 'buy' ? t('import.buy') : t('import.sell') }}
              </Badge>
            </td>
            <td class="px-2 py-1.5 text-right tabular-nums">
              {{ imp.side === 'buy' ? imp.qty_bought : imp.qty_sold }}
            </td>
            <td class="px-2 py-1.5 text-right tabular-nums">{{ imp.price }}</td>
            <td class="px-2 py-1.5 text-right">
              <div class="flex justify-end gap-1">
                <Button
                  variant="ghost"
                  size="icon"
                  :title="t('import.confirmOne')"
                  :disabled="importBusy"
                  @click="confirmImports([imp.exec_id])"
                >
                  <Check class="size-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  :title="t('import.discardOne')"
                  :disabled="importBusy"
                  @click="discardImports([imp.exec_id])"
                >
                  <X class="size-4 text-destructive" />
                </Button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
