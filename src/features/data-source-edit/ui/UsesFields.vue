<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { CurrencyPicker } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { FormField } from '@/shared/ui/form-field'

import type { SourceDraftState } from '../model/use-source-draft'

/**
 * Where the value goes: a currency's inflation, an asset's price, a
 * deposit's rate (asked before it is applied). Scenarios pick the source
 * themselves in their node; they are listed, not edited, here.
 */
const props = defineProps<{ state: SourceDraftState }>()
const { t } = useI18n()

const tree = useStoreTree()
const deposits = computed(() =>
  flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived && node.store.deposit),
)
const uses = computed(() => props.state.uses.value)
</script>

<template>
  <fieldset class="grid gap-4 sm:grid-cols-2" data-tour="source-uses">
    <legend class="mb-2 text-sm font-medium">{{ t('sources.uses.title') }}</legend>
    <div class="grid content-start gap-1">
      <CurrencyPicker
        id="source-inflation"
        v-model="uses.inflation"
        :label="t('sources.uses.inflation')"
        :kinds="['fiat']"
        allow-empty
      />
      <p class="text-xs text-muted-foreground">{{ t('sources.uses.inflationHint') }}</p>
    </div>
    <FormField for="source-deposit" :label="t('sources.uses.deposit')" :hint="t('sources.uses.depositHint')">
      <StoreNodeSelect
        id="source-deposit"
        v-model="uses.depositStoreId"
        :nodes="deposits"
        flat
        :empty-label="t('sources.uses.none')"
      />
    </FormField>
    <div class="grid content-start gap-1">
      <AssetInput v-model="uses.asset" :label="t('sources.uses.asset')" />
      <p class="text-xs text-muted-foreground">
        {{ uses.assetKey && !uses.asset ? t('sources.uses.assetKept', { key: uses.assetKey }) : t('sources.uses.assetHint') }}
      </p>
    </div>
    <CurrencyPicker
      v-if="uses.asset || uses.assetKey"
      id="source-price-currency"
      v-model="uses.priceCurrency"
      :label="t('sources.uses.priceCurrency')"
      :kinds="['fiat']"
    />
    <p v-if="props.state.workflowUses.value.length" class="text-xs text-muted-foreground sm:col-span-2">
      {{ t('sources.uses.workflows', { names: props.state.workflowUses.value.map((use) => (use.kind === 'workflow' ? use.name : '')).join(', ') }) }}
    </p>
  </fieldset>
</template>
