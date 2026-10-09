<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { X } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'

import type { TransactionDeleteProps } from './transaction-delete.types'
import { useTransactionDelete } from './useTransactionDelete'

const props = defineProps<TransactionDeleteProps>()

const { t } = useI18n()
const remove = useTransactionDelete()
</script>

<template>
  <Button
    variant="ghost"
    size="icon"
    class="shrink-0"
    :title="t('tx.delete')"
    :disabled="remove.isPending.value"
    @click="remove.mutate(props.id)"
  >
    <span class="sr-only">{{ t('tx.delete') }}</span>
    <X class="size-4 text-destructive" />
  </Button>
</template>
