<script setup lang="ts">
import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { AssetInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'

import { useAccountAsset } from '../../model/use-account-asset'
import type { ConvertMode, SplitRow } from '../../transaction-form.types'
import { AccountRefPicker } from '../account-ref-picker'

const props = defineProps<{ kind: TxKind; mode: ConvertMode; index: number; giveAsset: AssetRef | null }>()
const row = defineModel<SplitRow>({ required: true })
const emit = defineEmits<{ remove: [] }>()

const { t } = useI18n()

// What arrives on a money-only account is its currency: picked, not searched.
const to = computed(() => row.value.to)
const { currency } = useAccountAsset(to)
let auto: AssetRef | null = null
watch(currency, (value) => {
  if (props.kind !== 'convert' || !value) return
  if (!row.value.asset || row.value.asset === auto) {
    auto = value
    row.value.asset = value
  }
})
</script>

<template>
  <div class="grid gap-2 rounded-md border border-border p-2.5">
    <div class="flex items-center justify-between gap-2">
      <span class="text-xs font-medium text-muted-foreground">{{ t('tx.split.recipient', { n: props.index + 2 }) }}</span>
      <Button type="button" variant="ghost" size="icon" class="size-7" :aria-label="t('actions.delete')" @click="emit('remove')">
        <Trash2 class="size-3.5" />
      </Button>
    </div>
    <template v-if="props.kind === 'convert'">
      <div class="grid gap-2 @sm:grid-cols-2">
        <div v-if="props.mode === 'p2p'" class="grid gap-1.5">
          <Label>{{ t('tx.p2p.via') }}</Label>
          <AccountRefPicker v-model="row.via" :stores="false" />
        </div>
        <div class="grid gap-1.5">
          <Label>{{ props.mode === 'p2p' ? t('tx.p2p.receiveTo') : t('tx.field.receive') }}</Label>
          <AccountRefPicker v-model="row.to" :contacts="props.mode !== 'p2p'" :raw="props.mode !== 'p2p'" />
        </div>
      </div>
      <div class="grid gap-2 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label>{{ t('tx.split.give', { asset: props.giveAsset?.symbol ?? '' }) }}</Label>
          <AmountInput v-model="row.give" />
        </div>
        <AssetInput v-model="row.asset" :label="t('form.asset')" />
        <div class="grid gap-1.5">
          <Label>{{ t('tx.split.receive') }}</Label>
          <AmountInput v-model="row.receive" />
        </div>
      </div>
    </template>
    <div v-else class="grid gap-2 @sm:grid-cols-[1fr_10rem]">
      <div class="grid gap-1.5">
        <Label>{{ t('tx.split.account') }}</Label>
        <AccountRefPicker v-model="row.to" :contacts="false" :raw="false" />
      </div>
      <div class="grid gap-1.5">
        <Label>{{ t('form.amount') }}</Label>
        <AmountInput v-model="row.receive" />
      </div>
    </div>
  </div>
</template>
