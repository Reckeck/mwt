<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import { useBudgetDelete } from './model/use-budget-delete'

/** Full-New-Fixes 4: a limit goes only after a question, and comes back from the toast's «Отменить». */
const props = defineProps<{ budget: Budget | null; name: string }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ deleted: [] }>()

const { t } = useI18n()
const remove = useBudgetDelete()

function confirm() {
  if (!props.budget) return
  remove.mutate(
    { budget: props.budget, name: props.name },
    {
      onSuccess: () => {
        open.value = false
        emit('deleted')
      },
    },
  )
}
</script>

<template>
  <Dialog overlay="budget-delete" v-model:open="open" size="sm" :title="t('budget.deleteTitle', { name: props.name })">
    <p class="px-4 pb-2 text-sm text-muted-foreground" data-budget-delete-text>{{ t('budget.deleteText') }}</p>
    <template #footer>
      <Button variant="outline" class="max-md:h-11" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button variant="destructive" class="max-md:h-11" :disabled="remove.isPending.value" data-budget-delete-confirm @click="confirm">
        {{ t('actions.delete') }}
      </Button>
    </template>
  </Dialog>
</template>
