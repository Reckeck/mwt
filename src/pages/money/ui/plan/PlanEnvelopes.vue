<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { ComboStoreNodeSelect } from '@/entities/store'
import { EnvelopeManager } from '@/features/envelope-manage'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'
import { EmptyState } from '@/shared/ui/empty-state'
import { FormField } from '@/shared/ui/form-field'
import { SectionState } from '@/shared/ui/section-state'

import { envelopeAssets, hasEnvelopes } from '../../lib/envelope-assets'

/** Envelopes put money aside inside one wallet, so the section first asks which wallet. */
const { t } = useI18n()
const settings = useSettingsStore()
const tree = useStoreTree()

const wallets = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => hasEnvelopes(node.store)))
const storeId = ref('')
watch(
  wallets,
  (nodes) => {
    if (!nodes.some((node) => node.store.id === storeId.value)) storeId.value = nodes[0]?.store.id ?? ''
  },
  { immediate: true },
)

const overview = useQuery({
  // no placeholder: another wallet's positions
  queryKey: computed(() => ['store', storeId.value, true]),
  queryFn: () => call<StoreOverview>('get_store_overview', { storeId: storeId.value, includeChildren: true }),
  enabled: computed(() => storeId.value !== ''),
})
const assets = computed(() =>
  overview.data.value
    ? envelopeAssets(overview.data.value.store, overview.data.value.positions.map((position) => position.asset), settings.baseCurrency)
    : [],
)
</script>

<template>
  <section class="grid content-start gap-3 rounded-lg border border-border bg-card p-4" data-tour="plan-envelopes">
    <h2 class="heading-section">{{ t('money.plan.envelopes') }}</h2>
    <SectionState :loading="tree.isPending.value" :error="tree.error.value" skeleton="list" compact @retry="tree.refetch()">
      <EmptyState v-if="!wallets.length" compact :title="t('money.plan.envelopesNoWallets')" />
      <div v-else class="grid gap-3">
        <FormField for="envelope-wallet" :label="t('money.plan.envelopesWallet')">
          <ComboStoreNodeSelect id="envelope-wallet" v-model="storeId" :nodes="wallets" />
        </FormField>
        <EnvelopeManager v-if="overview.data.value" :key="storeId" :store-id="storeId" :assets="assets" />
      </div>
    </SectionState>
  </section>
</template>
