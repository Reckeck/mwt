<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { ConfirmButton } from '@/shared/ui/confirm-button'

import type { TransactionDeleteProps } from './transaction-delete.types'
import { useTransactionDelete } from './useTransactionDelete'

const props = defineProps<TransactionDeleteProps>()

const { t } = useI18n()
const remove = useTransactionDelete()
</script>

<template>
  <ConfirmButton
    :label="t('tx.delete')"
    :confirm-label="t('tx.deleteConfirm')"
    :description="t('confirm.undoable')"
    :disabled="remove.isPending.value"
    :show-label="props.labelled"
    @confirm="remove.mutate(props.id)"
  >
    <Trash2 class="size-4" />
  </ConfirmButton>
</template>
