<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { emptyLiabilityTerms, LiabilityTermsFields, StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'

import { updateInputOf, useStoreUpdate } from '../model/use-store-update'
import type { StoreFormProps } from '../store-edit.types'

const props = defineProps<StoreFormProps>()
const { t } = useI18n()

const terms = ref<LiabilityTerms>(emptyLiabilityTerms())
const securedBy = ref('')
const isLoan = computed(() => props.store.kind === 'liability')

const tree = useQuery({
  queryKey: STORE_TREE_QUERY_KEY,
  queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
  enabled: isLoan,
})
const properties = computed(() =>
  (tree.data.value ?? []).filter((node) => node.store.kind === 'property' && !node.store.archived),
)

watch(
  () => props.store,
  (store) => {
    terms.value = store.liability ? { ...store.liability } : emptyLiabilityTerms()
    securedBy.value = store.secured_by ?? ''
  },
  { immediate: true },
)

const save = useStoreUpdate()
</script>

<template>
  <form
    class="grid gap-4"
    @submit.prevent="
      save.mutate(updateInputOf(props.store, { liability: terms, secured_by: isLoan ? securedBy || null : null }))
    "
  >
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
      <Button type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
