<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'
import {
  Archive,
  ArchiveRestore,
  Calculator,
  Key,
  MoreVertical,
  Plus,
  RefreshCw,
  Trash2,
  X,
} from 'lucide-vue-next'

import { ExchangeKeyForm, useExchangeKeyStatus } from '@/features/exchange-key'
import { PendingImports } from '@/features/pending-imports'
import { TransactionDelete } from '@/features/transaction-delete'
import { PlanCalculator, type CalculatorPlan } from '@/features/plan-calculator'
import { AddTransactionForm } from '@/features/transaction-create'
import { StoreChart } from '@/entities/store'
import { StoreHistory } from '@/entities/store'
import { StorePositions } from '@/entities/store'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Dialog } from '@/shared/ui/dialog'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { Tabs } from '@/shared/ui/tabs'
import { formatDate, formatMoney } from '@/shared/lib/format'
import { call, parseError, type AppError } from '@/shared/api/ipc'
import { cn } from '@/shared/lib/utils'
import { getStoreIcon } from '@/shared/lib/store-icons'
import { txId } from '@/shared/lib/transactions'
import { useSettingsStore } from '@/shared/store/settings'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const queryClient = useQueryClient()
const settingsStore = useSettingsStore()

const storeId = computed(() => String(route.params.id ?? ''))
const formRef = ref<InstanceType<typeof AddTransactionForm> | null>(null)

const overview = useQuery({
  queryKey: ['store', storeId.value],
  queryFn: () => call<StoreOverview>('get_store_overview', { storeId: storeId.value }),
})

const pnl = useQuery({
  queryKey: ['store-pnl', storeId.value],
  queryFn: () => call<RealizedPnl>('get_store_pnl', { storeId: storeId.value }),
})

const base = computed(() => overview.data.value?.base_currency ?? '')
const store = computed(() => overview.data.value?.store)
const isArchived = computed(() => store.value?.archived === true)
const decimals = computed(() => settingsStore.vault?.display_decimals ?? 8)
const positions = computed(() => overview.data.value?.positions ?? [])

const money = (value: string) => formatMoney(value, { currency: base.value, decimals: 2 })

/** Backend failures surface with their own code so the cause is visible. */
const errorText = computed(() => {
  const failure = overview.error.value as AppError | null
  if (!failure) return t('wallet.notFound')
  const known = t(`errors.${failure.code}`)
  return known === `errors.${failure.code}` ? failure.message : known
})

function pnlTone(value: string): string {
  const amount = Number(value)
  if (amount > 0) return 'text-emerald-600 dark:text-emerald-400'
  if (amount < 0) return 'text-destructive'
  return ''
}

/* --- modal + tab state --- */
const dialogOpen = ref(false)
const dialogTab = ref<'transaction' | 'calculator'>('transaction')
const tab = ref('positions')

const tabs = computed(() => [
  { value: 'positions', label: t('pos.title') },
  { value: 'history', label: t('hist.title') },
])

const dialogTabs = computed(() => [
  { value: 'transaction', label: t('wallet.operation') },
  { value: 'calculator', label: t('wallet.plan') },
])

const stats = computed(() => [
  {
    key: 'total',
    label: t('store.total'),
    value: money(overview.data.value?.total_base ?? '0'),
    hint: `${t('store.fxRate')}: ${formatMoney(overview.data.value?.fx_per_usd, { currency: base.value })}`,
    tone: '',
  },
  {
    key: 'positions',
    label: t('pos.title'),
    value: String(positions.value.length),
    hint:
      positions.value
        .slice(0, 3)
        .map((position) => position.asset.symbol.toUpperCase())
        .join(' · ') || '—',
    tone: '',
  },
  {
    key: 'pnl',
    label: t('wallet.realizedPnl'),
    value: money(pnl.data.value?.total_base ?? '0'),
    hint: `${t('wallet.trades')}: ${pnl.data.value?.legs.length ?? 0}`,
    tone: pnlTone(pnl.data.value?.total_base ?? '0'),
  },
  {
    key: 'tx',
    label: t('hist.title'),
    value: String(overview.data.value?.tx_count ?? 0),
    hint: overview.data.value?.last_tx_at ? formatDate(overview.data.value.last_tx_at) : '—',
    tone: '',
  },
])

const refreshPrices = useMutation({
  mutationFn: () => call<RefreshResult>('refresh_all'),
  onSuccess: (result) => {
    toast.success(t('wallet.pricesRefreshed'), {
      description: `${result.prices} · ${result.fx}`,
    })
    void queryClient.invalidateQueries({ queryKey: ['store', storeId.value] })
    void queryClient.invalidateQueries({ queryKey: ['store-history', storeId.value] })
    void queryClient.invalidateQueries({ queryKey: ['summary'] })
  },
  onError: (err) => toast.error(t('wallet.pricesRefreshFailed'), { description: parseError(err).message }),
})

const toggleArchive = useMutation({
  mutationFn: (archived: boolean) => call('archive_store', { id: storeId.value, archived }),
  onSuccess: (_result, archived) => {
    toast.success(archived ? t('wallet.archived') : t('wallet.unarchived'))
    void queryClient.invalidateQueries({ queryKey: ['store', storeId.value] })
    void queryClient.invalidateQueries({ queryKey: ['stores'] })
    void queryClient.invalidateQueries({ queryKey: ['summary'] })
  },
  onError: (err) => toast.error(t('wallet.archiveFailed'), { description: parseError(err).message }),
})

const destroy = useMutation({
  mutationFn: () => call('delete_store', { id: storeId.value }),
  onSuccess: () => {
    toast.success(t('wallet.deleted'))
    void queryClient.invalidateQueries({ queryKey: ['stores'] })
    void queryClient.invalidateQueries({ queryKey: ['summary'] })
    void router.push({ name: 'stores' })
  },
  onError: (err) => toast.error(t('wallet.deleteFailed'), { description: parseError(err).message }),
})

/* --- Bybit API key + import staging (only for exchange nodes) --- */
const isExchangeNode = computed(() => store.value?.kind === 'exchange')
const { hasKey: bybitHasKey } = useExchangeKeyStatus(storeId, isExchangeNode)

/** Calculator "create transaction" hand-off: prefill first, then switch to transaction tab. */
function onApplyPlan(payload: CalculatorPlan) {
  formRef.value?.prefill(payload)
  dialogTab.value = 'transaction'
}
</script>

<template>
  <div v-if="overview.isPending.value" class="py-16 text-center text-sm text-muted-foreground">
    {{ t('store.loading') }}
  </div>

  <div v-else-if="overview.isError.value || !store" class="py-16 text-center">
    <p class="text-sm text-destructive">
      {{ overview.isError.value ? errorText : t('wallet.notFound') }}
    </p>
    <p v-if="overview.isError.value" class="mt-1 break-all text-xs text-muted-foreground">
      {{ errorText }}
    </p>
    <Button variant="outline" size="sm" class="mt-3" @click="router.push({ name: 'stores' })">
      {{ t('wallet.backToStores') }}
    </Button>
  </div>

  <div v-else class="grid gap-6">
    <!-- Sticky header: identity, totals and every page action in one bar. -->
    <div
      class="sticky top-0 z-20 -mx-4 -mt-4 border-b border-border bg-background/95 px-4 py-3 backdrop-blur md:-mx-6 md:-mt-6 md:px-6"
    >
      <div class="flex flex-wrap items-center gap-x-4 gap-y-3">
        <span
          class="flex size-7 shrink-0 items-center justify-center rounded-md ring-1 ring-background"
          :style="{
            backgroundColor: `${store.color}1A`,
            color: store.color,
            borderColor: `${store.color}33`,
          }"
        >
          <component :is="getStoreIcon(store.kind)" class="size-4" />
        </span>

        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="truncate font-semibold">{{ store.name }}</h2>
            <Badge variant="secondary">{{ t(`stores.kind.${store.kind}`) }}</Badge>
            <Badge v-if="store.archived" variant="outline">{{ t('stores.archived') }}</Badge>
          </div>
          <p v-if="store.note" class="truncate text-xs text-muted-foreground">{{ store.note }}</p>
        </div>

        <div class="shrink-0 text-right">
          <p class="text-xl font-bold tabular-nums">{{ money(overview.data.value?.total_base ?? '0') }}</p>
          <p class="text-xs text-muted-foreground">
            ≈ {{ formatMoney(overview.data.value?.total_usd, { currency: 'usd' }) }}
          </p>
        </div>

        <div class="flex shrink-0 items-center gap-1">
          <Button size="sm" :disabled="isArchived" @click="dialogTab = 'transaction'; dialogOpen = true">
            <Plus class="size-4" />
            {{ t('wallet.addOperation') }}
          </Button>

          <Button variant="outline" size="icon" :title="t('calc.title')" @click="dialogTab = 'calculator'; dialogOpen = true">
            <Calculator class="size-4" />
          </Button>

          <Button
            variant="ghost"
            size="icon"
            :title="t('store.refresh')"
            :disabled="refreshPrices.isPending.value"
            @click="refreshPrices.mutate()"
          >
            <RefreshCw class="size-4" :class="cn(refreshPrices.isPending.value && 'animate-spin')" />
          </Button>

          <DropdownMenu align="end">
            <template #trigger>
              <Button variant="ghost" size="icon" :title="t('wallet.more')">
                <MoreVertical class="size-4" />
              </Button>
            </template>
            <DropdownMenuItem
              :value="store.archived ? 'unarchive' : 'archive'"
              @select="toggleArchive.mutate(!store.archived)"
            >
              <component :is="store.archived ? ArchiveRestore : Archive" class="size-4 shrink-0" />
              {{ store.archived ? t('stores.unarchive') : t('stores.archive') }}
            </DropdownMenuItem>
            <DropdownMenuItem value="delete" destructive @select="destroy.mutate()">
              <Trash2 class="size-4 shrink-0" />
              {{ t('stores.delete') }}
            </DropdownMenuItem>
          </DropdownMenu>
        </div>
      </div>

      <p v-if="isArchived" class="mt-2 rounded-md border border-border bg-muted/50 p-2 text-xs text-muted-foreground">
        {{ t('wallet.archivedHint') }}
      </p>
    </div>

    <!-- Stats strip -->
    <div class="grid grid-cols-2 gap-4 lg:grid-cols-4">
      <Card v-for="stat in stats" :key="stat.key" class="p-4">
        <p class="truncate text-xs text-muted-foreground">{{ stat.label }}</p>
        <p class="mt-1 truncate text-lg font-bold tabular-nums" :class="stat.tone">{{ stat.value }}</p>
        <p class="truncate text-xs text-muted-foreground">{{ stat.hint }}</p>
      </Card>
    </div>

    <!-- Bybit API key section (only for exchange nodes) -->
    <Card v-if="isExchangeNode" class="mt-4">
      <template #header>
        <div class="flex items-center gap-2">
          <Key class="size-4 text-primary" />
          <p class="font-medium">{{ t('wallet.bybit.title') }}</p>
        </div>
      </template>
      <div class="grid gap-3">
        <ExchangeKeyForm :store-id="storeId" />
        <PendingImports v-if="bybitHasKey" :store-id="storeId" />
      </div>
    </Card>

    <StoreChart :store-id="storeId" :base-currency="base" />

    <div v-if="positions.length === 0" class="rounded-xl border border-dashed border-border p-8 text-center">
      <p class="text-sm font-medium">{{ t('wallet.empty') }}</p>
      <p class="mt-1 text-sm text-muted-foreground">{{ t('wallet.emptyHint') }}</p>
      <Button class="mt-4" :disabled="isArchived" @click="dialogTab = 'transaction'; dialogOpen = true">
        <Plus class="size-4" />
        {{ t('wallet.addOperation') }}
      </Button>
    </div>

    <Tabs v-else v-model="tab" :tabs="tabs">
      <template #positions>
        <StorePositions :positions="positions" :base-currency="base" :decimals="decimals" />
      </template>
      <template #history>
        <StoreHistory :store-id="storeId">
          <template #actions="{ tx }">
            <TransactionDelete :id="txId(tx)" />
          </template>
        </StoreHistory>
      </template>
    </Tabs>

    <!-- Unified dialog with tabs for Transaction form and Calculator -->
    <Dialog v-model:open="dialogOpen" :title="dialogTab === 'transaction' ? t('wallet.operation') : t('wallet.plan')" size="lg">
      <template #close>
        <Button variant="ghost" size="icon" @click="dialogOpen = false">
          <X class="size-4" />
          <span class="sr-only">{{ t('actions.close') }}</span>
        </Button>
      </template>
      <Tabs v-model="dialogTab" :tabs="dialogTabs" class="w-full">
        <template #transaction>
          <AddTransactionForm ref="formRef" :store-id="storeId" :disabled="isArchived" />
        </template>
        <template #calculator>
          <PlanCalculator :base-currency="base" @apply="onApplyPlan" />
        </template>
      </Tabs>
    </Dialog>
  </div>
</template>
