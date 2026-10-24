<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { FormField } from '@/shared/ui/form-field'
import { Combobox, type ComboboxOption } from '@/shared/ui/combobox'
import { StoreNodeSelect } from '@/entities/store'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryLabel } from '@/shared/lib/categories'
import { flattenStoreTree, withoutLoyalty } from '@/shared/lib/store-tree'

/** Categories a reminder pays: what is spent, not income or service. */
const PAY_GROUPS: CategoryGroup[] = ['fixed', 'daily', 'savings']

/**
 * «Кнопка в напоминании» (Auto-2 · 10): «Оплатить «Квартира › ЖКХ»» opens the
 * payment form filled with this category and wallet. `null` — no button.
 */
const pay = defineModel<ReminderPay | null>({ required: true })

const { t } = useI18n()
const categories = useCategories()
const tree = useStoreTree()

const nodes = computed(() => withoutLoyalty(tree.data.value ?? []))
const storeName = computed(() => {
  const id = pay.value?.store_id
  return id ? flattenStoreTree(nodes.value).find((node) => node.store.id === id)?.store.name : undefined
})

const options = computed<ComboboxOption[]>(() => [
  { value: '', label: t('reminder.payNone') },
  ...PAY_GROUPS.flatMap((group) =>
    (categories.data.value ?? [])
      .filter((category) => category.group === group && (!category.archived || category.key === pay.value?.category))
      .map((category) => {
        const name = categoryLabel(category, t)
        return {
          value: category.key,
          label: t('reminder.payCategory', { name: storeName.value ? `${storeName.value} › ${name}` : name }),
          group,
          groupLabel: t(`categoryGroup.${group}`),
        }
      }),
  ),
])

const category = computed({
  get: () => pay.value?.category ?? '',
  set: (key: string) => {
    pay.value = key ? { store_id: pay.value?.store_id ?? null, category: key } : null
  },
})
const storeId = computed({
  get: () => pay.value?.store_id ?? '',
  set: (id: string) => {
    if (pay.value) pay.value = { ...pay.value, store_id: id || null }
  },
})
</script>

<template>
  <FormField for="reminder-pay" :label="t('reminder.payButton')" :hint="t('reminder.payHint')" data-tour="reminder-pay">
    <Combobox id="reminder-pay" v-model="category" :options="options" :placeholder="t('reminder.payNone')" />
  </FormField>
  <FormField v-if="pay" for="reminder-pay-store" :label="t('reminder.payStore')">
    <StoreNodeSelect id="reminder-pay-store" v-model="storeId" :nodes="nodes" :empty-label="t('reminder.payStoreAny')" />
  </FormField>
</template>
