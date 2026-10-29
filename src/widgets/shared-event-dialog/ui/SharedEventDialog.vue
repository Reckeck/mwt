<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CategorySelect } from '@/features/category-select'
import { ContactAccountForm } from '@/features/contact-edit'
import { SharedEventEditDialog, type SharedEventEditDialogProps } from '@/features/shared-event-edit'

/**
 * `New event` / `Edit event` with what the event form borrows from other features: the
 * category picker and the requisite form of a guest. The dialog itself lives in `shared-event-edit`.
 */
const props = defineProps<SharedEventEditDialogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [view: SharedEventView] }>()
const { t } = useI18n()
</script>

<template>
  <SharedEventEditDialog v-model:open="open" :event="props.event" @saved="(view) => emit('saved', view)">
    <template #category="{ value, update }">
      <CategorySelect
        :model-value="value"
        :groups="['daily', 'fixed']"
        :placeholder="t('budget.pickCategory')"
        @update:model-value="(key) => update(String(key))"
      />
    </template>
    <template #requisite-form="{ submit, cancel }">
      <ContactAccountForm :groups="[]" @submit="submit" @cancel="cancel" />
    </template>
  </SharedEventEditDialog>
</template>
