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
  Check,
  Download,
  Key,
  MoreVertical,
  Plus,
  RefreshCw,
  Trash2,
  X,
} from 'lucide-vue-next'

import AddTransactionForm from '@/components/wallet/AddTransactionForm.vue'
import WalletCalculator from '@/components/wallet/WalletCalculator.vue'
import WalletChart from '@/components/wallet/WalletChart.vue'
import WalletHistory from '@/components/wallet/WalletHistory.vue'
import WalletPositions from '@/components/wallet/WalletPositions.vue'
import Badge from '@/components/ui/badge.vue'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Dialog from '@/components/ui/dialog.vue'
import DropdownMenu from '@/components/ui/dropdown-menu.vue'
import DropdownMenuItem from '@/components/ui/dropdown-menu-item.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Tabs from '@/components/ui/tabs.vue'
import { formatDate, formatMoney } from '@/lib/format'
import { call, parseError, type AppError } from '@/lib/ipc'
import { cn } from '@/lib/utils'
import { getStoreIcon } from '@/lib/store-icons'
import { useSettingsStore } from '@/stores/settings'

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
const decimals = computed(() => settingsStore.settings?.display_decimals ?? 8)
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

/* --- Bybit API key management (only for exchange nodes) --- */
const isExchangeNode = computed(() => store.value?.kind === 'exchange')
const bybitKey = ref('')
const bybitSecret = ref('')
const bybitBusy = ref(false)

const keyStatus = useQuery({
  queryKey: ['bybit-key-status', storeId.value],
  queryFn: () => call<ExchangeKeyStatus>('exchange_key_status', { storeId: storeId.value }),
  enabled: isExchangeNode,
})

const bybitHasKey = computed(() => keyStatus.data.value?.configured === true)
const bybitHint = computed(() => keyStatus.data.value?.hint ?? '')

async function saveBybitKey() {
  bybitBusy.value = true
  try {
    await call('set_exchange_key', {
      input: { store_id: storeId.value, api_key: bybitKey.value, api_secret: bybitSecret.value },
    })
    bybitKey.value = ''
    bybitSecret.value = ''
    toast.success(t('wallet.bybit.keySaved'))
    await keyStatus.refetch()
  } catch (err) {
    toast.error(t('wallet.bybit.keySaveFailed'), { description: parseError(err).message })
  } finally {
    bybitBusy.value = false
  }
}

async function removeBybitKey() {
  bybitBusy.value = true
  try {
    await call('clear_exchange_key', { storeId: storeId.value })
    toast.success(t('wallet.bybit.keyRemoved'))
    await keyStatus.refetch()
  } catch (err) {
    toast.error(t('wallet.bybit.keyRemoveFailed'), { description: parseError(err).message })
  } finally {
    bybitBusy.value = false
  }
}

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
      input: { store_id: storeId.value },
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
      ['store', storeId.value],
      ['store-pnl', storeId.value],
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

/** Calculator "create transaction" hand-off: prefill first, then switch to transaction tab. */
function onApplyPlan(payload: { mode: CalcMode; from: AssetRef | null; to: AssetRef | null; amount: string; price: string }) {
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
        <p class="text-sm text-muted-foreground">{{ t('wallet.bybit.hint') }}</p>

        <div v-if="bybitHasKey" class="flex items-center gap-2">
          <Badge variant="secondary">{{ bybitHint }}</Badge>
          <Button variant="ghost" :disabled="bybitBusy" @click="removeBybitKey">
            {{ t('wallet.bybit.remove') }}
          </Button>
        </div>

        <template v-else>
          <div class="grid gap-1.5">
            <Label for="bk">{{ t('wallet.bybit.apiKey') }}</Label>
            <Input id="bk" v-model="bybitKey" autocomplete="off" spellcheck="false" />
          </div>
          <div class="grid gap-1.5">
            <Label for="bs">{{ t('wallet.bybit.apiSecret') }}</Label>
            <Input
              id="bs"
              v-model="bybitSecret"
              type="password"
              autocomplete="new-password"
              spellcheck="false"
            />
          </div>
          <div class="flex items-center gap-3">
            <Button :disabled="bybitBusy || !bybitKey || !bybitSecret" @click="saveBybitKey">
              {{ t('wallet.bybit.save') }}
            </Button>
          </div>
        </template>

        <!-- D2.4: import history into a staging area, confirmed explicitly -->
        <div v-if="bybitHasKey" class="grid gap-3 border-t border-border pt-3">
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
      </div>
    </Card>

    <WalletChart :store-id="storeId" :base-currency="base" />

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
        <WalletPositions :positions="positions" :base-currency="base" :decimals="decimals" />
      </template>
      <template #history>
        <WalletHistory :store-id="storeId" />
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
          <WalletCalculator :base-currency="base" @apply="onApplyPlan" />
        </template>
      </Tabs>
    </Dialog>
  </div>
</template>
