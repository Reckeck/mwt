<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Archive, ArchiveRestore, Calculator, MoreVertical, Plus, RefreshCw, Trash2, X } from 'lucide-vue-next'

import AddTransactionForm from '@/components/wallet/AddTransactionForm.vue'
import WalletCalculator from '@/components/wallet/WalletCalculator.vue'
import WalletChart from '@/components/wallet/WalletChart.vue'
import WalletHistory from '@/components/wallet/WalletHistory.vue'
import WalletPositions from '@/components/wallet/WalletPositions.vue'
import Badge from '@/components/ui/badge.vue'
import Button from '@/components/ui/button.vue'
import Dialog from '@/components/ui/dialog.vue'
import DropdownMenu from '@/components/ui/dropdown-menu.vue'
import DropdownMenuItem from '@/components/ui/dropdown-menu-item.vue'
import Tabs from '@/components/ui/tabs.vue'
import { formatDate, formatMoney } from '@/lib/format'
import { call, type AppError } from '@/lib/ipc'
import { cn } from '@/lib/utils'
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

const money = (v: string) => formatMoney(v, { currency: base.value, decimals: 2 })

/** Backend failures surface with their own code so the cause is visible. */
const errorText = computed(() => {
  const e = overview.error.value as AppError | null
  if (!e) return t('wallet.notFound')
  const known = t(`errors.${e.code}`)
  return known === `errors.${e.code}` ? e.message : known
})

function pnlTone(v: string): string {
  const n = Number(v)
  if (n > 0) return 'text-emerald-600 dark:text-emerald-400'
  if (n < 0) return 'text-destructive'
  return ''
}

/* --- modal + tab state --- */
const txDialogOpen = ref(false)
const calcDialogOpen = ref(false)
const tab = ref('positions')

const tabs = computed(() => [
  { value: 'positions', label: t('pos.title') },
  { value: 'history', label: t('hist.title') },
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
    hint: positions.value
      .slice(0, 3)
      .map((p) => p.asset.symbol.toUpperCase())
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
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['store', storeId.value] })
    void queryClient.invalidateQueries({ queryKey: ['store-history', storeId.value] })
    void queryClient.invalidateQueries({ queryKey: ['summary'] })
  },
})

const toggleArchive = useMutation({
  mutationFn: (archived: boolean) => call('archive_store', { id: storeId.value, archived }),
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['store', storeId.value] })
    void queryClient.invalidateQueries({ queryKey: ['stores'] })
    void queryClient.invalidateQueries({ queryKey: ['summary'] })
  },
})

const destroy = useMutation({
  mutationFn: () => call('delete_store', { id: storeId.value }),
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['stores'] })
    void queryClient.invalidateQueries({ queryKey: ['summary'] })
    void router.push({ name: 'stores' })
  },
})

/** Calculator "create transaction" hand-off: prefill first, then reveal the form. */
function onApplyPlan(payload: { mode: CalcMode; from: AssetRef | null; to: AssetRef | null; amount: string; price: string }) {
  calcDialogOpen.value = false
  formRef.value?.prefill(payload)
  txDialogOpen.value = true
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

  <div v-else class="grid gap-5">
    <!-- Sticky header: identity, totals and every page action in one bar. -->
    <div
      class="sticky top-0 z-20 -mx-4 -mt-4 border-b border-border bg-background/95 px-4 py-3 backdrop-blur md:-mx-6 md:-mt-6 md:px-6"
    >
      <div class="flex flex-wrap items-center gap-x-4 gap-y-3">
        <span
          class="size-3 shrink-0 rounded-full ring-2 ring-background"
          :style="{ backgroundColor: store.color }"
        />

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
          <Button size="sm" :disabled="isArchived" @click="txDialogOpen = true">
            <Plus class="size-4" />
            {{ t('wallet.addOperation') }}
          </Button>

          <Button variant="outline" size="icon" :title="t('calc.title')" @click="calcDialogOpen = true">
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

    <!-- One compact strip instead of four stacked cards. -->
    <div class="grid grid-cols-2 gap-px overflow-hidden rounded-xl border border-border bg-border lg:grid-cols-4">
      <div v-for="s in stats" :key="s.key" class="bg-card p-3">
        <p class="truncate text-xs text-muted-foreground">{{ s.label }}</p>
        <p class="mt-0.5 truncate text-lg font-bold tabular-nums" :class="s.tone">{{ s.value }}</p>
        <p class="truncate text-xs text-muted-foreground">{{ s.hint }}</p>
      </div>
    </div>

    <WalletChart :store-id="storeId" :base-currency="base" />

    <div v-if="positions.length === 0" class="rounded-xl border border-dashed border-border p-8 text-center">
      <p class="text-sm font-medium">{{ t('wallet.empty') }}</p>
      <p class="mt-1 text-sm text-muted-foreground">{{ t('wallet.emptyHint') }}</p>
      <Button class="mt-4" :disabled="isArchived" @click="txDialogOpen = true">
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

    <!-- Transaction form -->
    <Dialog
      v-model:open="txDialogOpen"
      :title="t('wallet.operation')"
      size="lg"
    >
      <template #close>
        <Button variant="ghost" size="icon" @click="txDialogOpen = false">
          <X class="size-4" />
          <span class="sr-only">{{ t('actions.close') }}</span>
        </Button>
      </template>
      <AddTransactionForm ref="formRef" :store-id="storeId" :disabled="isArchived" />
    </Dialog>

    <!-- Calculator -->
    <Dialog v-model:open="calcDialogOpen" :title="t('wallet.plan')" size="lg">
      <template #close>
        <Button variant="ghost" size="icon" @click="calcDialogOpen = false">
          <X class="size-4" />
          <span class="sr-only">{{ t('actions.close') }}</span>
        </Button>
      </template>
      <WalletCalculator :base-currency="base" @apply="onApplyPlan" />
    </Dialog>
  </div>
</template>
