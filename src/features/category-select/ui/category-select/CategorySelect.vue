<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { Combobox, type ComboboxOption } from '@/shared/ui/combobox'
import { call } from '@/shared/api/ipc'
import { categoryLabel } from '@/shared/lib/categories'

import type { CategorySelectProps } from './category-select.types'

const props = defineProps<CategorySelectProps>()

/** Category key(s): a list with `multiple`, a single key otherwise. */
const model = defineModel<string | string[]>({ required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const categories = useQuery({
  queryKey: ['categories'],
  queryFn: () => call<Category[]>('list_categories'),
})

const pickedKeys = computed(() => (Array.isArray(model.value) ? model.value : [model.value]))

/** Live categories of the offered groups, plus any archived one that is already picked. */
const options = computed<ComboboxOption[]>(() =>
  props.groups.flatMap((group) =>
    (categories.data.value ?? [])
      .filter(
        (category) => category.group === group && (!category.archived || pickedKeys.value.includes(category.key)),
      )
      .map((category) => ({
        value: category.key,
        label: categoryLabel(category, t),
        group,
        groupLabel: t(`categoryGroup.${group}`),
      })),
  ),
)

const create = useMutation({
  mutationFn: (name: string) =>
    call<Category>('create_category', {
      input: { key: null, name, group: props.createGroup ?? props.groups[0] ?? 'daily' },
    }),
  onSuccess: (category) => {
    queryClient.setQueryData<Category[]>(['categories'], (old) => [...(old ?? []), category])
    void queryClient.invalidateQueries({ queryKey: ['categories'] })
    model.value = Array.isArray(model.value) ? [...model.value, category.key] : category.key
  },
  meta: {
    error: 'categorySelect.createFailed',
    success: (data) => t('categorySelect.created', { name: categoryLabel(data as Category, t) }),
  },
})

const createLabel = computed(() =>
  props.noCreate ? undefined : (text: string) => t('categorySelect.create', { name: text }),
)
</script>

<template>
  <Combobox
    v-model="model"
    :id="props.id"
    :options="options"
    :multiple="props.multiple"
    :placeholder="props.placeholder ?? t('categorySelect.placeholder')"
    :aria-label="props.ariaLabel"
    :empty-label="t('categorySelect.empty')"
    :create-label="createLabel"
    :disabled="create.isPending.value"
    @create="(name) => create.mutate(name)"
  />
</template>
