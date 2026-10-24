<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { required, useFormValidation } from '@/shared/lib/form'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'

import { EDITABLE_GROUPS } from '../category-manager.constants'

/** «+ Категория»: a name and a group; the new row lands in its group below. */
const props = defineProps<{ pending: boolean }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ create: [input: { name: string; group: CategoryGroup }] }>()

const { t } = useI18n()
const name = ref('')
const group = ref<CategoryGroup>('daily')
const form = useFormValidation(() => ({ name: required(name.value) }))

watch(open, (shown) => {
  if (!shown) return
  name.value = ''
  group.value = 'daily'
  form.reset()
})
</script>

<template>
  <FormDialog
    v-model:open="open"
    form-id="category-add-form"
    :title="t('categories.manage.addTitle')"
    :description="t('categories.manage.hint')"
    :pending="props.pending"
    @submit="form.submit(() => emit('create', { name, group }), $event)"
  >
    <FormField :label="t('categories.manage.name')" required :error="form.error('name')">
      <Input v-model="name" :placeholder="t('categories.manage.newName')" :aria-label="t('categories.manage.name')" />
    </FormField>
    <FormField :label="t('categories.manage.group')">
      <Select v-model="group" :aria-label="t('categories.manage.group')">
        <SelectItem v-for="item in EDITABLE_GROUPS" :key="item" :value="item">{{ t(`categoryGroup.${item}`) }}</SelectItem>
      </Select>
    </FormField>
  </FormDialog>
</template>
