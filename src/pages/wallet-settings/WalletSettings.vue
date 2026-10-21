<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

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
import { ImportSettingsForm } from '@/features/exchange-import-settings'
import { StoreAssetClassField } from '@/features/asset-class-edit'
import { AutomationList } from '@/widgets/automation-list'
import { ExchangeImported } from '@/widgets/exchange-imported'
import { ExchangeSyncHistory } from '@/widgets/exchange-sync-history'
import { presetsOf, useWalletCatalog } from '@/entities/store'
import { Badge } from '@/shared/ui/badge'
import { type PageCrumb, PageHeader } from '@/shared/ui/page-header'
import { SectionState } from '@/shared/ui/section-state'
import { SettingsSection } from '@/shared/ui/settings-section'
import { Tabs } from '@/shared/ui/tabs'
import { call } from '@/shared/api/ipc'
import { fiatRef } from '@/shared/lib/asset-ref'
import { storeIcon } from '@/shared/lib/store-icons'
import { useSettingsStore } from '@/shared/store/settings'

import { sectionId, useSettingsGroups } from './model/use-settings-groups'
import { SubAccountList } from './ui/sub-account-list'

const route = useRoute()
const { t } = useI18n()
const settings = useSettingsStore()
const catalog = useWalletCatalog()

const storeId = computed(() => String(route.params.id ?? ''))

const overview = useQuery({
  // no placeholder: another wallet's settings
  queryKey: computed(() => ['store', storeId.value, true]),
  queryFn: () => call<StoreOverview>('get_store_overview', { storeId: storeId.value, includeChildren: true }),
})

const store = computed(() => overview.data.value?.store)
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

/** Money and investments take a basket; a home, points and loans do not. */
const hasBasket = computed(() => !!store.value && !['property', 'loyalty', 'liability'].includes(store.value.kind))

/** Ready-made automations this kind of wallet offers. */
const presets = computed(() => (store.value ? presetsOf(catalog.data.value, store.value) : []))

const createOpen = ref(false)
const moveTarget = ref<Store | null>(null)
const moveOpen = computed({
  get: () => moveTarget.value !== null,
  set: (value: boolean) => {
    if (!value) moveTarget.value = null
  },
})

const { groups, group, isRoot, showSection } = useSettingsGroups(store)

const crumbs = computed<PageCrumb[]>(() =>
  store.value
    ? [
        { label: t('nav.stores'), to: { name: 'stores' } },
        { label: store.value.name, to: { name: 'wallet', params: { id: store.value.id } } },
        { label: t('walletSettings.nav') },
      ]
    : [],
)
</script>

<template>
  <SectionState
    :loading="overview.isPending.value"
    :error="overview.error.value"
    :empty="!store"
    :empty-title="t('wallet.notFound')"
    skeleton="list"
    @retry="overview.refetch()"
  >
    <div v-if="store" class="flex w-full flex-col gap-4">
      <PageHeader :crumbs="crumbs">
        <component :is="storeIcon(store)" class="store-tint size-5 shrink-0" :style="{ '--store-color': store.color }" />
        <h2 class="heading-page min-w-0 truncate">{{ store.name }}</h2>
        <Badge variant="secondary">{{ store.sub_type ? t(`subType.${store.sub_type}`) : t(`stores.kind.${store.kind}`) }}</Badge>
      </PageHeader>

      <!-- Five groups at most; a setting that does not apply to this kind is not shown. -->
      <Tabs v-model="group" :tabs="groups">
        <template #general>
          <div class="flex flex-col gap-4">
            <SettingsSection :title="t('walletSettings.general')"><StoreGeneralForm :store="store" /></SettingsSection>
            <SettingsSection
              v-if="store.kind === 'property' && isRoot"
              :id="sectionId('property')"
              :title="t('walletSettings.property')"
            >
              <StorePropertyForm :store="store" />
            </SettingsSection>
            <SettingsSection v-if="store.kind === 'loyalty'" :id="sectionId('loyalty')" :title="t('walletSettings.loyalty')">
              <LoyaltyForm :store="store" />
            </SettingsSection>
            <SettingsSection
              v-if="store.sub_type === 'savings' || store.sub_type === 'deposit' || store.sub_type === 'pds'"
              :id="sectionId('deposit')"
              :title="t('walletSettings.deposit')"
            >
              <DepositTermsForm :store="store" />
            </SettingsSection>
            <SettingsSection v-if="hasBasket" :id="sectionId('basket')" :title="t('walletSettings.assetClass')">
              <StoreAssetClassField :store="store" />
            </SettingsSection>
            <SettingsSection
              v-if="store.kind === 'liability' || store.sub_type === 'credit_card'"
              :id="sectionId('terms')"
              :title="t('walletSettings.terms')"
            >
              <StoreTermsForm :store="store" />
            </SettingsSection>
          </div>
        </template>

        <template #accounts>
          <div class="flex flex-col gap-4">
            <SubAccountList
              v-if="isRoot"
              :children="children"
              :base-currency="overview.data.value?.base_currency"
              :archived="store.archived"
              @create="createOpen = true"
              @move="(child) => (moveTarget = child)"
            />
            <SettingsSection :title="t('walletSettings.envelopes')">
              <EnvelopeManager :store-id="store.id" :assets="envelopeAssets" />
            </SettingsSection>
          </div>
        </template>

        <template #automations>
          <AutomationList :store-id="store.id" :presets="presets" />
        </template>

        <template #exchange>
          <div class="flex flex-col gap-4">
            <SettingsSection :title="t('walletSettings.exchange')">
              <StoreExchangeForm :store="store" @configure="showSection('import')" />
            </SettingsSection>
            <SettingsSection :id="sectionId('import')" :title="t('walletSettings.import')">
              <ImportSettingsForm :store-id="store.id" />
            </SettingsSection>
            <SettingsSection :id="sectionId('syncs')" :title="t('walletSettings.syncs')">
              <ExchangeSyncHistory :store-id="store.id" />
            </SettingsSection>
            <SettingsSection :id="sectionId('imported')" :title="t('walletSettings.imported')">
              <ExchangeImported :store-id="store.id" />
            </SettingsSection>
          </div>
        </template>

        <template #danger>
          <StoreDangerZone :store="store" :child-count="children.length" :tx-count="overview.data.value?.tx_count ?? 0" />
        </template>
      </Tabs>

      <StoreCreateDialog v-model:open="createOpen" :parent="store" :parent-path="[store.name]" />
      <StoreChangeDialog v-if="moveTarget" v-model:open="moveOpen" :store="moveTarget" initial="move_to" />
    </div>
  </SectionState>
</template>
