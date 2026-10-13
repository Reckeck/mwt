<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Check, Download, X } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'

import type { PendingImportsProps } from './pending-imports.types'
import { useExchangeSync } from './use-exchange-sync'

const props = withDefaults(defineProps<PendingImportsProps>(), { storeId: '', limit: 0, compact: false })

const { t } = useI18n()
const sync = useExchangeSync()

// -- D2.4: Bybit import staging -----------------------------------------
// Fills are staged in the vault and only become transactions when the user
// confirms them, so an import can never silently rewrite history.
const pending = useQuery({
  queryKey: ['pending-imports'],
  queryFn: () => call<ListPendingImportsOutput>('list_pending_imports'),
})

/** Only this exchange's records (all of them with no `storeId`), newest first; the widget shows the first few. */
const rows = computed(() => {
  const all = pending.data.value?.imports ?? []
  const mine = props.storeId ? all.filter((imp) => imp.store_id === props.storeId) : all
  return props.limit ? mine.slice(0, props.limit) : mine
})
const pendingCount = computed(() => {
  const all = pending.data.value?.imports ?? []
  return props.storeId ? all.filter((imp) => imp.store_id === props.storeId).length : all.length
})
const busy = computed(() => sync.syncNow.isPending.value || sync.confirm.isPending.value || sync.discard.isPending.value)
/** «Подтвердить все» confirms what this list shows the owner of: one exchange, or every one. */
const allIds = computed(() =>
  props.storeId
    ? (pending.data.value?.imports ?? []).filter((imp) => imp.store_id === props.storeId).map((imp) => imp.exec_id)
    : [],
)
</script>

<template>
  <!-- D2.4: import history into a staging area, confirmed explicitly -->
  <div class="grid gap-3" :class="!props.compact && 'border-t border-border pt-3'">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p v-if="!props.compact" class="text-sm font-medium">
        {{ t('import.title') }}
        <Badge v-if="pendingCount" variant="secondary">{{ pendingCount }}</Badge>
      </p>
      <div class="flex gap-2">
        <Button
          v-if="props.storeId && !props.compact"
          variant="outline"
          size="sm"
          :disabled="busy"
          @click="sync.syncNow.mutate(props.storeId)"
        >
          <Download class="size-4" />
          {{ t('import.syncNow') }}
        </Button>
        <Button
          v-if="pendingCount"
          size="sm"
          :disabled="busy"
          @click="sync.confirm.mutate(allIds)"
        >
          <Check class="size-4" />
          {{ t('import.confirmAll') }}
        </Button>
      </div>
    </div>
    <p v-if="!props.compact" class="text-xs text-muted-foreground">{{ t('import.hint') }}</p>
    <p v-if="pending.isSuccess.value && !pendingCount" class="text-sm text-muted-foreground">{{ t('import.nothingPending') }}</p>

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
          <tr v-for="imp in rows" :key="imp.exec_id">
            <td class="px-2 py-1.5 font-mono text-xs">{{ imp.occurred_at.slice(0, 10) }}</td>
            <td class="px-2 py-1.5">{{ imp.symbol }}</td>
            <td class="px-2 py-1.5">
              <Badge v-if="imp.kind === 'fill'" :variant="imp.side === 'buy' ? 'default' : 'secondary'">
                {{ imp.side === 'buy' ? t('import.buy') : t('import.sell') }}
              </Badge>
              <!-- A move between Bybit accounts or an Earn payout: what it is and where it goes. -->
              <span v-else class="flex flex-wrap items-center gap-1">
                <Badge variant="outline">{{ t(`pending.kind.${imp.kind}`) }}</Badge>
                <span class="text-xs text-muted-foreground">
                  <template v-if="imp.kind === 'transfer'">
                    {{ t(`exchangeBalances.account.${imp.from_account}`) }} → {{ t(`exchangeBalances.account.${imp.to_account}`) }}
                  </template>
                  <template v-else>{{ t('exchangeBalances.account.EARN') }}</template>
                </span>
              </span>
            </td>
            <td class="px-2 py-1.5 text-right tabular-nums">
              {{ imp.kind !== 'fill' || imp.side === 'buy' ? imp.qty_bought : imp.qty_sold }}
            </td>
            <td class="px-2 py-1.5 text-right tabular-nums">{{ imp.kind === 'fill' ? imp.price : '—' }}</td>
            <td class="px-2 py-1.5 text-right">
              <div class="flex justify-end gap-1">
                <Button
                  variant="ghost"
                  size="icon"
                  :title="t('import.confirmOne')"
                  :disabled="busy"
                  @click="sync.confirm.mutate([imp.exec_id])"
                >
                  <Check class="size-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  :title="t('import.discardOne')"
                  :disabled="busy"
                  @click="sync.discard.mutate([imp.exec_id])"
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
