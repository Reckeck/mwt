<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { Textarea } from '@/shared/ui/textarea'
import { required, useFormValidation } from '@/shared/lib/form'

import { useWorkflowActions } from '../../model/use-workflow-actions'

/** Paste a workflow someone shared (its text from «Скопировать как файл»). */
const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ imported: [workflow: Workflow] }>()

const { t } = useI18n()
const { importFile } = useWorkflowActions()
const text = ref('')
watch(open, (value) => {
  if (value) text.value = ''
})
const form = useFormValidation(() => ({ text: required(text.value) }), { resetOn: open })

function submit() {
  importFile.mutate(text.value, {
    onSuccess: (saved) => {
      open.value = false
      emit('imported', saved.workflow)
    },
  })
}
</script>

<template>
  <Dialog v-model:open="open" :title="t('workflow.import.title')" :description="t('workflow.import.description')" size="lg">
    <form id="workflow-import" class="grid gap-4" @submit.prevent="form.submit(submit, $event)">
      <FormField :label="t('workflow.import.label')" for="workflow-import-text" required :error="form.error('text')">
        <Textarea id="workflow-import-text" v-model="text" rows="10" class="font-mono text-xs" />
      </FormField>
    </form>
    <template #footer>
      <Button type="submit" form="workflow-import" :disabled="importFile.isPending.value">
        {{ t('workflow.import.submit') }}
      </Button>
    </template>
  </Dialog>
</template>
