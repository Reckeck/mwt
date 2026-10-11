<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'

import type { ConvertMode, SplitRow } from '../../transaction-form.types'
import { AccountRefPicker } from '../account-ref-picker'

/** One more account money arrives on: the account and how much lands there. */
const props = defineProps<{ kind: TxKind; mode: ConvertMode; index: number }>()
const row = defineModel<SplitRow>({ required: true })
const emit = defineEmits<{ remove: [] }>()

const { t } = useI18n()
</script>

<template>
  <div class="grid items-start gap-2 @sm:grid-cols-[minmax(0,1fr)_10rem_2rem]">
    <AccountRefPicker
      v-model="row.to"
      :contacts="props.kind === 'convert' && props.mode !== 'p2p'"
      :raw="false"
      :aria-label="t('tx.split.accountN', { n: props.index + 2 })"
    />
    <AmountInput v-model="row.receive" :placeholder="t('form.amount')" :aria-label="t('form.amount')" />
    <Button type="button" variant="ghost" size="icon" class="size-9" :aria-label="t('actions.delete')" @click="emit('remove')">
      <Trash2 class="size-4" />
    </Button>
  </div>
</template>
