<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Pencil } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'

import TransactionEditDialog from './TransactionEditDialog.vue'
import type { TransactionEditButtonProps } from './transaction-edit.types'

const props = withDefaults(defineProps<TransactionEditButtonProps>(), { storeId: undefined })

const { t } = useI18n()
const open = ref(false)
</script>

<template>
  <Button
    variant="ghost"
    size="icon"
    class="size-8 shrink-0 text-muted-foreground"
    :title="t('tx.edit')"
    :aria-label="t('tx.edit')"
    @click="open = true"
  >
    <Pencil class="size-4" />
  </Button>
  <TransactionEditDialog v-model:open="open" :transaction="props.tx" :store-id="props.storeId" />
</template>
