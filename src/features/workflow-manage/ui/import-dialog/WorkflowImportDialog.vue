<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Label } from '@/shared/ui/label'
import { Textarea } from '@/shared/ui/textarea'

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
const missing = computed(() => (text.value.trim() ? '' : t('workflow.import.missing')))

function submit() {
  if (missing.value) return
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
    <form id="workflow-import" class="grid gap-1.5" @submit.prevent="submit">
      <Label for="workflow-import-text">{{ t('workflow.import.label') }}</Label>
      <Textarea id="workflow-import-text" v-model="text" rows="10" class="font-mono text-xs" />
      <p v-if="missing" class="text-xs text-muted-foreground">{{ missing }}</p>
    </form>
    <template #footer>
      <Button type="submit" form="workflow-import" :disabled="Boolean(missing) || importFile.isPending.value">
        {{ t('workflow.import.submit') }}
      </Button>
    </template>
  </Dialog>
</template>
