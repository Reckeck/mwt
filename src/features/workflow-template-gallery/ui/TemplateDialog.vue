<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import { positiveAmount, required, useFormValidation, type FieldError } from '@/shared/lib/form'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'

import { useTemplateActions } from '../model/use-template-actions'
import TemplateFieldInput from './TemplateFieldInput.vue'

/**
 * The 3–5 fields of a template with the vault's values filled in; «Создать»
 * saves an ordinary workflow and opens it in the editor.
 */
const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{ template: WorkflowTemplateInfo | null }>()
const { t } = useI18n()
const router = useRouter()
const { create } = useTemplateActions()

const values = ref<Record<string, string>>({})
watch(open, (value) => {
  if (!value || !props.template) return
  values.value = Object.fromEntries(props.template.fields.map((field) => [field.key, field.default]))
})

function check(field: TemplateField): FieldError {
  const value = values.value[field.key] ?? ''
  if (field.required && !value.trim()) return required(value)
  return field.kind === 'number' ? positiveAmount(value) : null
}
const form = useFormValidation(
  () => Object.fromEntries((props.template?.fields ?? []).map((field) => [field.key, check(field)])),
  { resetOn: open },
)

function save() {
  if (!props.template) return
  create.mutate(
    { template_id: props.template.id, params: { ...values.value } },
    {
      onSuccess: (saved) => {
        open.value = false
        void router.push({ name: 'workflow-edit', params: { id: saved.workflow.id } })
      },
    },
  )
}
</script>

<template>
  <FormDialog overlay="template-dialog"
    v-model:open="open"
    :title="props.template ? t(`workflowTemplates.${props.template.id}.title`) : ''"
    :description="props.template ? t(`workflowTemplates.${props.template.id}.desc`) : undefined"
    form-id="workflow-template-form"
    :submit-label="t('workflowTemplates.create')"
    :pending="create.isPending.value"
    @submit="form.submit(save, $event)"
  >
    <template v-if="props.template">
      <FormField
        v-for="field in props.template.fields"
        :key="field.key"
        :for="`template-${field.key}`"
        :label="t(`workflowTemplates.field.${field.key}`)"
        :required="field.required"
        :error="form.error(field.key)"
      >
        <TemplateFieldInput
          :id="`template-${field.key}`"
          v-model="values[field.key]"
          :field="field"
          :template-id="props.template.id"
          :invalid="!!form.error(field.key)"
        />
      </FormField>
      <p v-if="props.template.moves_money" class="text-sm text-muted-foreground">{{ t('workflowTemplates.confirmHint') }}</p>
    </template>
  </FormDialog>
</template>
