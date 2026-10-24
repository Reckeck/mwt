<script setup lang="ts">
import { computed, toRef } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Check, RefreshCw } from 'lucide-vue-next'

import { useExchangeKeyStatus } from '@/features/exchange-key'
import { PendingImports, useExchangeSync } from '@/features/pending-imports'
import { ExchangeBalances } from '@/widgets/exchange-balances'
import { Button, buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'
import { Switch } from '@/shared/ui/switch'
import { useIsPhone } from '@/shared/lib/phone'

import { formatDayMonthTime } from '../../lib/import-time'
import { HOURLY, useExchangeImport } from '../../model/use-exchange-import'

/**
 * The exchange's import (Wallets-2 · 8): the connection on the left — the key,
 * read only, its status, «Импортировать сейчас», the hourly import — and on the
 * right what the last import brought and how it avoids duplicates. What waits
 * for confirmation and the balances check follow below.
 */
const props = defineProps<{ store: Store; hasKey: boolean }>()
const { t } = useI18n()
const phone = useIsPhone()
const sync = useExchangeSync()
const { hint } = useExchangeKeyStatus(
  computed(() => props.store.id),
  computed(() => props.hasKey),
)
const { lastRun, lastOk, interval, toggle, saving } = useExchangeImport(toRef(props, 'store'))

/** «•••• 7Q2f»: the key's last four, never the key. */
const masked = computed(() => `•••• ${hint.value.replace(/^.*…/, '').slice(-4)}`)
const status = computed(() => {
  const run = lastRun.value
  if (!run) return t('exchangeImport.connected')
  if (run.error) return t('exchangeImport.failed', { when: formatDayMonthTime(run.at) })
  return t('exchangeImport.connectedAt', { when: formatDayMonthTime(run.at) })
})
/** «каждый час» when hourly or off; another interval set in the settings names itself. */
const autoLabel = computed(() =>
  interval.value === 0 || interval.value === HOURLY
    ? t('exchangeImport.hourly')
    : t('exchangeImport.every', { count: Math.round(interval.value / 60) }),
)
/** A run from before the split counted only `staged`: all of it reads as trades. */
const counts = computed(() => {
  const run = lastOk.value
  if (!run) return null
  const split = run.trades + run.transfers > 0
  return { trades: split ? run.trades : run.staged, transfers: run.transfers, known: run.known }
})
const ROW = 'flex items-center justify-between gap-3 py-1.5 text-sm'
</script>

<template>
  <div v-if="props.hasKey" class="flex flex-col gap-4">
    <div class="grid gap-4 @md:grid-cols-2" data-exchange-import>
      <component :is="phone ? 'div' : Card" class="flex flex-col gap-3">
        <h2 v-if="!phone" class="font-semibold">{{ t('exchangeImport.connection') }}</h2>
        <div class="flex flex-col gap-1.5">
          <span class="text-sm font-medium">{{ t('exchangeImport.key') }}</span>
          <div class="flex h-10 items-center rounded-md border border-input bg-muted/40 px-3 text-sm tabular-nums">{{ masked }}</div>
          <span class="text-xs text-muted-foreground">{{ phone ? t('exchangeImport.readOnly') : t('exchangeImport.readOnlyLong') }}</span>
        </div>
        <div v-if="!phone" class="flex items-center gap-2">
          <Check class="size-4 shrink-0" :class="lastRun?.error ? 'text-destructive' : 'text-success'" />
          <span class="min-w-0 flex-1 text-sm" :class="lastRun?.error && 'text-destructive'">{{ status }}</span>
          <Button variant="outline" size="sm" :disabled="sync.syncNow.isPending.value" @click="sync.syncNow.mutate(props.store.id)">
            <RefreshCw v-if="sync.syncNow.isPending.value" class="size-4 animate-spin" />
            {{ t('exchangeImport.now') }}
          </Button>
        </div>
        <div class="flex items-center justify-between gap-3 text-sm">
          <label for="exchange-auto-import">{{ autoLabel }}</label>
          <Switch id="exchange-auto-import" :model-value="interval > 0" :disabled="saving" @update:model-value="toggle" />
        </div>
        <Button
          v-if="phone"
          class="h-11 w-full"
          :disabled="sync.syncNow.isPending.value"
          @click="sync.syncNow.mutate(props.store.id)"
        >
          {{ t('exchangeImport.now') }}
        </Button>
      </component>

      <Card class="flex flex-col gap-1">
        <h2 v-if="!phone" class="font-semibold">{{ t('exchangeImport.last') }}</h2>
        <p v-if="!counts" class="text-sm text-muted-foreground">{{ t('exchangeImport.never') }}</p>
        <template v-else>
          <div :class="ROW"><span class="text-muted-foreground">{{ t('exchangeImport.trades') }}</span><span class="tabular-nums">{{ counts.trades }}</span></div>
          <div v-if="!phone" :class="ROW">
            <span class="text-muted-foreground">{{ t('exchangeImport.transfers') }}</span><span class="tabular-nums">{{ counts.transfers }}</span>
          </div>
          <div :class="ROW"><span class="text-muted-foreground">{{ t('exchangeImport.duplicates') }}</span><span class="tabular-nums">{{ counts.known }}</span></div>
        </template>
        <p v-if="!phone" class="mt-2 flex items-start gap-2 rounded-lg bg-muted px-3 py-2.5 text-sm">
          <Check class="mt-0.5 size-4 shrink-0" />
          {{ t('exchangeImport.dedupe') }}
        </p>
      </Card>
    </div>
    <Card><PendingImports :store-id="props.store.id" /></Card>
    <ExchangeBalances :store-id="props.store.id" />
  </div>
  <EmptyState v-else :title="t('wallet.importNeedsKey')">
    <RouterLink
      :to="{ name: 'wallet-settings', params: { id: props.store.id }, query: { tab: 'exchange' } }"
      :class="buttonVariants({ variant: 'outline', size: 'sm' })"
    >
      {{ t('wallet.openExchangeSettings') }}
    </RouterLink>
  </EmptyState>
</template>
