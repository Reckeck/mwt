<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { StoreNodeSelect } from '@/entities/store'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryOptions } from '@/shared/lib/categories'
import { pluralForm } from '@/shared/lib/format'
import { required, useFormValidation } from '@/shared/lib/form'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { Combobox } from '@/shared/ui/combobox'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'

import type { BulkPickDialogProps } from './bulk-pick-dialog.types'

const props = defineProps<BulkPickDialogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ pick: [value: string, name: string] }>()

const { t, locale } = useI18n()
const categories = useCategories()
const tree = useStoreTree()

const value = ref('')
watch(open, (isOpen) => {
  if (isOpen) value.value = ''
})

const options = computed(() => categoryOptions(categories.data.value, t, ['daily', 'fixed', 'income', 'savings', 'service']))
/** A property holds no money, so nothing moves onto it. */
const storeNodes = computed(() =>
  flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived && node.store.kind !== 'property'),
)

const title = computed(() =>
  t(`bulk.${props.kind}Title.${pluralForm(props.count, locale.value)}`, { count: props.count }),
)
const form = useFormValidation(() => ({ value: required(value.value, 'pick') }), { resetOn: open })

function save() {
  const name =
    props.kind === 'category'
      ? (options.value.find((option) => option.value === value.value)?.label ?? value.value)
      : (storeNodes.value.find((node) => node.store.id === value.value)?.store.name ?? '')
  emit('pick', value.value, name)
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="title"
    size="sm"
    form-id="transaction-bulk-form"
    :submit-label="t('bulk.apply')"
    :pending="props.pending"
    @submit="form.submit(save, $event)"
  >
    <FormField
      for="bulk-value"
      :label="props.kind === 'category' ? t('bulk.category') : t('bulk.wallet')"
      required
      :error="form.error('value')"
    >
      <Combobox
        v-if="props.kind === 'category'"
        id="bulk-value"
        v-model="value"
        :options="options"
        :placeholder="t('categorySelect.placeholder')"
        :empty-label="t('categorySelect.empty')"
      />
      <StoreNodeSelect v-else id="bulk-value" v-model="value" :nodes="storeNodes" />
    </FormField>
  </FormDialog>
</template>
