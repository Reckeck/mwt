<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ArrowLeft, FolderInput, FolderPlus } from 'lucide-vue-next'

import { DepositTermsForm } from '@/features/deposit-terms'
import { LoyaltyForm } from '@/features/loyalty-edit'
import { EnvelopeManager } from '@/features/envelope-manage'
import { StoreCreateDialog } from '@/features/store-create'
import {
  StoreDangerZone,
  StoreExchangeForm,
  StoreGeneralForm,
  StorePropertyForm,
  StoreTermsForm,
} from '@/features/store-edit'
import { StoreChangeDialog } from '@/features/store-change'
import { AutomationList } from '@/widgets/automation-list'
import { bindingLabel, kindInfo, useWalletCatalog } from '@/entities/store'
import { Badge } from '@/shared/ui/badge'
import { Button, buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Tabs } from '@/shared/ui/tabs'
import { call } from '@/shared/api/ipc'
import { fiatRef } from '@/shared/lib/asset-ref'
import { formatMoney } from '@/shared/lib/format'
import { storeIcon } from '@/shared/lib/store-icons'
import { useSettingsStore } from '@/shared/store/settings'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const settings = useSettingsStore()
const catalog = useWalletCatalog()

const storeId = computed(() => String(route.params.id ?? ''))

const overview = useQuery({
  queryKey: computed(() => ['store', storeId.value, true]),
  queryFn: () => call<StoreOverview>('get_store_overview', { storeId: storeId.value, includeChildren: true }),
})

const store = computed(() => overview.data.value?.store)
const isRoot = computed(() => !store.value?.parent_id)
const children = computed(() => overview.data.value?.children ?? [])

/** Assets an envelope may earmark: the binding, else what the store holds, else the base currency. */
const envelopeAssets = computed<AssetRef[]>(() => {
  const current = store.value
  if (!current) return []
  if (current.binding !== 'any' && 'asset' in current.binding) return [current.binding.asset]
  if (current.binding !== 'any' && 'pair' in current.binding) {
    return [current.binding.pair.base, current.binding.pair.quote]
  }
  const held = (overview.data.value?.positions ?? []).map((position) => position.asset)
  return held.length ? held : [fiatRef(settings.baseCurrency)]
})

/** Tabs depend on the kind: a setting that does not apply is not shown at all. */
const tabs = computed(() => {
  const current = store.value
  if (!current) return []
  const list = [{ value: 'general', label: t('walletSettings.general') }]
  // Sub-accounts and envelopes make no sense for a kind that takes no sub-accounts (a home, a car).
  const takesChildren = (kindInfo(catalog.data.value, current.kind)?.sub_types.length ?? 0) > 0
  if (takesChildren) {
    list.push({ value: 'accounts', label: isRoot.value ? t('walletSettings.accounts') : t('walletSettings.envelopes') })
  }
  list.push({ value: 'automations', label: t('walletSettings.automations') })
  if (current.kind === 'exchange' && isRoot.value) list.push({ value: 'exchange', label: t('walletSettings.exchange') })
  if (current.kind === 'property' && isRoot.value) {
    list.push({ value: 'property', label: t('walletSettings.property') })
  }
  if (current.kind === 'loyalty') list.push({ value: 'loyalty', label: t('walletSettings.loyalty') })
  if (current.sub_type === 'savings' || current.sub_type === 'deposit' || current.sub_type === 'pds') {
    list.push({ value: 'deposit', label: t('walletSettings.deposit') })
  }
  if (current.kind === 'liability' || current.sub_type === 'credit_card') {
    list.push({ value: 'terms', label: t('walletSettings.terms') })
  }
  list.push({ value: 'danger', label: t('walletSettings.danger') })
  return list
})

/** The open tab lives in the URL, so a link can point straight at «Exchange». */
const tab = computed({
  get: () => {
    const wanted = String(route.query.tab ?? 'general')
    return tabs.value.some((entry) => entry.value === wanted) ? wanted : 'general'
  },
  set: (value: string) => void router.replace({ query: { ...route.query, tab: value } }),
})

/** Ready-made automations this kind of wallet offers. */
const presets = computed(() => (store.value ? (kindInfo(catalog.data.value, store.value.kind)?.presets ?? []) : []))

const createOpen = ref(false)
const moveTarget = ref<Store | null>(null)
const moveOpen = computed({
  get: () => moveTarget.value !== null,
  set: (value: boolean) => {
    if (!value) moveTarget.value = null
  },
})

const money = (value: string) => formatMoney(value, { currency: overview.data.value?.base_currency, decimals: 2 })
</script>

<template>
  <div v-if="overview.isPending.value" class="py-16 text-center text-sm text-muted-foreground">
    {{ t('store.loading') }}
  </div>
  <div v-else-if="!store" class="py-16 text-center text-sm text-destructive">{{ t('wallet.notFound') }}</div>

  <div v-else class="mx-auto flex w-full max-w-3xl flex-col gap-4">
    <div class="flex items-center gap-3">
      <RouterLink
        :to="{ name: 'wallet', params: { id: store.id } }"
        :class="buttonVariants({ variant: 'ghost', size: 'icon' })"
        class="size-8"
        :title="t('walletSettings.back')"
      >
        <ArrowLeft class="size-4" />
        <span class="sr-only">{{ t('walletSettings.back') }}</span>
      </RouterLink>
      <component :is="storeIcon(store)" class="size-5" :style="{ color: store.color }" />
      <h2 class="min-w-0 flex-1 truncate text-lg font-semibold">
        {{ t('walletSettings.title', { name: store.name }) }}
      </h2>
      <Badge variant="secondary">{{ store.sub_type ? t(`subType.${store.sub_type}`) : t(`stores.kind.${store.kind}`) }}</Badge>
    </div>

    <Tabs v-model="tab" :tabs="tabs">
      <template #general>
        <Card><StoreGeneralForm :store="store" /></Card>
      </template>

      <template #accounts>
        <div class="flex flex-col gap-4">
          <Card v-if="isRoot">
            <template #header>
              <div class="flex items-center justify-between gap-2">
                <p class="font-medium">{{ t('stores.subAccounts') }}</p>
                <Button size="sm" variant="outline" :disabled="store.archived" @click="createOpen = true">
                  <FolderPlus class="size-4" />
                  {{ t('stores.newSubAccountShort') }}
                </Button>
              </div>
            </template>
            <p v-if="!children.length" class="text-sm text-muted-foreground">{{ t('walletSettings.noAccounts') }}</p>
            <ul v-else class="flex flex-col divide-y divide-border">
              <li v-for="child in children" :key="child.store.id" class="flex items-center gap-3 py-2.5">
                <component :is="storeIcon(child.store)" class="size-4 shrink-0" :style="{ color: child.store.color }" />
                <RouterLink
                  :to="{ name: 'wallet-settings', params: { id: child.store.id } }"
                  class="min-w-0 flex-1 truncate text-sm font-medium hover:underline"
                >
                  {{ child.store.name }}
                </RouterLink>
                <Badge v-if="bindingLabel(child.store.binding)" variant="outline">{{ bindingLabel(child.store.binding) }}</Badge>
                <span class="shrink-0 text-sm tabular-nums">{{ money(child.subtree_base) }}</span>
                <Button
                  variant="ghost"
                  size="icon"
                  class="size-8"
                  :title="t('stores.move')"
                  :aria-label="t('stores.move')"
                  @click="moveTarget = child.store"
                >
                  <FolderInput class="size-4" />
                </Button>
              </li>
            </ul>
          </Card>
          <Card>
            <template #header>
              <p class="font-medium">{{ t('walletSettings.envelopes') }}</p>
            </template>
            <EnvelopeManager :store-id="store.id" :assets="envelopeAssets" />
          </Card>
        </div>
      </template>

      <template #automations>
        <AutomationList :store-id="store.id" :presets="presets" />
      </template>


      <template #loyalty>
        <Card><LoyaltyForm :store="store" /></Card>
      </template>

      <template #deposit>
        <Card><DepositTermsForm :store="store" /></Card>
      </template>

      <template #exchange>
        <Card><StoreExchangeForm :store="store" /></Card>
      </template>

      <template #property>
        <Card><StorePropertyForm :store="store" /></Card>
      </template>

      <template #terms>
        <Card><StoreTermsForm :store="store" /></Card>
      </template>

      <template #danger>
        <StoreDangerZone
          :store="store"
          :child-count="children.length"
          :tx-count="overview.data.value?.tx_count ?? 0"
        />
      </template>
    </Tabs>

    <StoreCreateDialog v-model:open="createOpen" :parent="store" :parent-path="[store.name]" />
    <StoreChangeDialog v-if="moveTarget" v-model:open="moveOpen" :store="moveTarget" initial="move_to" />
  </div>
</template>
