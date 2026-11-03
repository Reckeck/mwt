<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { emptyLiabilityTerms, LiabilityTermsFields, StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'
import { SyncChangedBanner } from '@/shared/ui/sync-changed-banner'

import { useStoreFormEdit } from '../model/use-store-form-edit'
import type { StoreFormProps } from '../store-edit.types'
import { useStoreTree } from '@/shared/api/queries'

const props = defineProps<StoreFormProps>()
const { t } = useI18n()

const terms = ref<LiabilityTerms>(emptyLiabilityTerms())
const securedBy = ref('')
const isLoan = computed(() => props.store.kind === 'liability')

const tree = useStoreTree(isLoan)
const properties = computed(() =>
  (tree.data.value ?? []).filter((node) => node.store.kind === 'property' && !node.store.archived),
)

interface TermsFields {
  terms: LiabilityTerms
  securedBy: string
}
const edit = useStoreFormEdit<TermsFields>({
  store: () => props.store,
  model: (store) => ({
    terms: store.liability ? { ...store.liability } : emptyLiabilityTerms(),
    securedBy: store.secured_by ?? '',
  }),
  current: () => ({ terms: { ...terms.value }, securedBy: securedBy.value }),
  load: (fields) => {
    terms.value = fields.terms
    securedBy.value = fields.securedBy
  },
})

function submit() {
  edit.save({ liability: terms.value, secured_by: isLoan.value ? securedBy.value || null : null })
}
</script>

<template>
  <form class="grid gap-4" @submit.prevent="submit">
    <SyncChangedBanner v-if="edit.changedElsewhere.value" @refresh="edit.refreshFromOtherDevice" />
    <LiabilityTermsFields v-model="terms" />
    <div v-if="isLoan" class="grid gap-1.5">
      <Label for="store-secured-by">{{ t('property.securedBy') }}</Label>
      <StoreNodeSelect
        id="store-secured-by"
        v-model="securedBy"
        :nodes="properties"
        flat
        :empty-label="t('property.securedByNone')"
      />
    </div>
    <div>
      <Button type="submit" :disabled="edit.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
