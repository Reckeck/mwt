<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { required, useFormValidation } from '@/shared/lib/form'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

/** Asks for the name a run is saved under; the parent does the saving. */
const props = defineProps<{ title: string; initial: string; pending: boolean }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ submit: [name: string] }>()

const { t } = useI18n()
const name = ref('')
watch(open, (now) => {
  if (now) name.value = props.initial
})
const form = useFormValidation(() => ({ name: required(name.value) }), { resetOn: open })
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="props.title"
    size="sm"
    form-id="calc-scenario-name"
    :submit-label="t('actions.save')"
    :pending="props.pending"
    @submit="form.submit(() => emit('submit', name.trim()), $event)"
  >
    <FormField for="calc-scenario-name-input" :label="t('invest.scenario.name')" required :error="form.error('name')">
      <Input id="calc-scenario-name-input" v-model="name" :placeholder="t('invest.scenario.namePlaceholder')" />
    </FormField>
  </FormDialog>
</template>
