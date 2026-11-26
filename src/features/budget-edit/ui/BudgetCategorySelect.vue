<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useCategories } from '@/shared/api/queries'
import { categoryLabel } from '@/shared/lib/categories'
import { Combobox, type ComboboxOption } from '@/shared/ui/combobox'

import { BUDGET_CATEGORY_GROUPS } from '../budget-edit.constants'

/** The category a limit watches: live categories of the spending groups, plus an archived one already picked. */
const category = defineModel<string>({ required: true })
const { t } = useI18n()
const categories = useCategories()

const options = computed<ComboboxOption[]>(() =>
  BUDGET_CATEGORY_GROUPS.flatMap((group) =>
    (categories.data.value ?? [])
      .filter((entry) => entry.group === group && (!entry.archived || entry.key === category.value))
      .map((entry) => ({
        value: entry.key,
        label: categoryLabel(entry, t),
        group,
        groupLabel: t(`categoryGroup.${group}`),
      })),
  ),
)
</script>

<template>
  <Combobox v-model="category" id="budget-category" :options="options" :placeholder="t('budget.pickCategory')" :empty-label="t('budget.pickCategory')" />
</template>
