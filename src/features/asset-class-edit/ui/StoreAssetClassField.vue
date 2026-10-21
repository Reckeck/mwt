<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AssetClassSelect } from '@/entities/asset-class'
import { useAssetClasses, useEmergencyStatus } from '@/shared/api/queries'
import { FormField } from '@/shared/ui/form-field'

import { useAssetClassAssign } from '../model/use-asset-class-edit'

/** The wallet's default basket: every asset in it goes there unless placed by hand. */
const props = defineProps<{ store: Store }>()
const { t } = useI18n()

const view = useAssetClasses()
const fund = useEmergencyStatus()
const assign = useAssetClassAssign()

/** A fund account stays outside the baskets (decision 35): the field says so instead of offering a pick. */
const inFund = computed(() => fund.data.value?.fund.scope.store_ids.includes(props.store.id) ?? false)

function pick(classId: string) {
  assign.mutate({
    target: { kind: 'store', store_id: props.store.id },
    classId: classId || null,
    previous: props.store.asset_class ?? null,
  })
}
</script>

<template>
  <FormField
    for="store-asset-class"
    :label="t('assetClass.store.label')"
    :hint="inFund ? t('assetClass.store.fund') : t('assetClass.store.hint')"
  >
    <AssetClassSelect
      id="store-asset-class"
      :classes="view.data.value?.classes ?? []"
      :model-value="inFund ? '' : (props.store.asset_class ?? '')"
      :disabled="inFund || assign.isPending.value || props.store.archived"
      @update:model-value="pick"
    />
  </FormField>
</template>
