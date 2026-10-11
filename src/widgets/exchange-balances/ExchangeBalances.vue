<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { RefreshCw } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call, parseError } from '@/shared/api/ipc'

/**
 * Bybit's own balances per account next to what the ledger booked on the
 * matching sub-account. A difference is history the import has not seen
 * (a deposit, a withdrawal, an older trade) — the user decides what it was.
 */
const props = defineProps<{ storeId: string }>()
const { t } = useI18n()

const balances = useQuery({
  queryKey: computed(() => ['bybit-balances', props.storeId]),
  queryFn: () => call<ExchangeAccountBalance[]>('bybit_balances', { storeId: props.storeId }),
  enabled: false,
  retry: false,
})
const tone = (diff: string) => (Number(diff) === 0 ? 'text-muted-foreground' : Number(diff) > 0 ? 'text-amber-600 dark:text-amber-400' : 'text-destructive')
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="font-medium">{{ t('exchangeBalances.title') }}</p>
        <Button size="sm" variant="outline" :disabled="balances.isFetching.value" @click="balances.refetch()">
          <RefreshCw class="size-4" :class="balances.isFetching.value && 'animate-spin'" />
          {{ t('exchangeBalances.load') }}
        </Button>
      </div>
    </template>
    <p v-if="balances.isError.value" class="text-sm text-destructive">
      {{ t('exchangeBalances.failed') }}: {{ parseError(balances.error.value).message }}
    </p>
    <div v-else-if="balances.data.value" class="grid gap-4 @lg:grid-cols-3">
      <section v-for="account in balances.data.value" :key="account.account" class="grid content-start gap-1.5">
        <p class="text-sm font-medium">{{ t(`exchangeBalances.account.${account.account}`) }}</p>
        <p v-if="!account.store_id" class="text-xs text-muted-foreground">{{ t('exchangeBalances.noAccount') }}</p>
        <p v-if="!account.rows.length" class="text-xs text-muted-foreground">{{ t('exchangeBalances.empty') }}</p>
        <table v-else class="w-full text-xs tabular-nums">
          <thead class="text-muted-foreground">
            <tr>
              <th class="text-left font-normal">{{ t('exchangeBalances.coin') }}</th>
              <th class="text-right font-normal">{{ t('exchangeBalances.exchange') }}</th>
              <th class="text-right font-normal">{{ t('exchangeBalances.ledger') }}</th>
              <th class="text-right font-normal">{{ t('exchangeBalances.diff') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in account.rows" :key="row.coin">
              <td class="font-medium">{{ row.coin }}</td>
              <td class="text-right">{{ row.exchange }}</td>
              <td class="text-right">{{ row.ledger }}</td>
              <td class="text-right" :class="tone(row.diff)">{{ Number(row.diff) === 0 ? '✓' : row.diff }}</td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
  </Card>
</template>
