<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'

import type { SplitRow } from '../../transaction-form.types'
import { AccountRefPicker } from '../account-ref-picker'

/** One more account money arrives on: the account and how much lands there. */
const props = defineProps<{ kind: TxKind; via?: boolean; index: number }>()
const row = defineModel<SplitRow>({ required: true })
const emit = defineEmits<{ remove: [] }>()

const { t } = useI18n()
</script>

<template>
  <div class="flex items-start gap-2">
    <div class="min-w-0 flex-1">
      <AccountRefPicker
        v-model="row.to"
        :contacts="props.kind === 'convert' && !props.via"
        :raw="false"
        :aria-label="t('tx.split.accountN', { n: props.index + 2 })"
      />
    </div>
    <AmountInput v-model="row.receive" class="w-24 shrink-0" :placeholder="t('form.amount')" :aria-label="t('form.amount')" />
    <Button type="button" variant="ghost" size="icon" :aria-label="t('actions.delete')" @click="emit('remove')">
      <Trash2 class="size-4" />
    </Button>
  </div>
</template>
