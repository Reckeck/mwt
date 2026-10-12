<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Tag } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { CurrencyPicker } from '@/entities/currency'
import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { AmountInput } from '@/shared/ui/amount-input'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { call, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

import type { AssetRevalueButtonProps } from './asset-revalue.types'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

const props = defineProps<AssetRevalueButtonProps>()

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const open = ref(false)
const price = ref('')
const currency = ref('')

const form = useFormValidation(
  () => ({ price: positiveAmount(price.value), currency: required(currency.value, 'pick') }),
  { resetOn: open },
)

watch(open, (isOpen) => {
  if (!isOpen) return
  price.value = ''
  currency.value = settings.baseCurrency
})

/** `AssetRef::key()` in Rust for a thing with no chain or contract. */
const assetKey = () => `asset:${props.asset.symbol}`

// The new value applies to the whole history, so every value view is stale.
const STALE = [...LEDGER_KEYS, ...PLANNING_KEYS]
const refresh = () => {
  for (const queryKey of STALE) void queryClient.invalidateQueries({ queryKey })
}

const save = useMutation({
  mutationFn: () =>
    call<ManualPrice>('set_manual_price', {
      input: { asset_key: assetKey(), unit_price: price.value.trim().replace(',', '.'), currency: currency.value },
    }),
  onSuccess: () => {
    toast.success(t('asset.revalued'))
    refresh()
    open.value = false
  },
  onError: (err) => toast.error(t('asset.revalueFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

const reset = useMutation({
  mutationFn: () => call('clear_manual_price', { assetKey: assetKey() }),
  onSuccess: () => {
    toast.success(t('asset.revalueCleared'))
    refresh()
    open.value = false
  },
  onError: (err) => toast.error(t('asset.revalueFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Button variant="ghost" size="sm" @click="open = true">
    <Tag class="size-4" />
    {{ t('asset.revalue') }}
  </Button>

  <Dialog v-model:open="open" :title="t('asset.revalueTitle', { name: props.asset.name || props.asset.symbol })" size="sm">
    <form id="asset-revalue-form" class="grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <div class="grid grid-cols-[1fr_8rem] gap-3">
        <FormField for="revalue-price" :label="t('tx.field.unitPrice')" required :error="form.error('price')">
          <AmountInput id="revalue-price" v-model="price" />
        </FormField>
        <FormField for="revalue-currency" :label="t('tx.field.currency')" required :error="form.error('currency')">
          <CurrencyPicker id="revalue-currency" v-model="currency" />
        </FormField>
      </div>
      <p class="text-xs text-muted-foreground">{{ t('asset.revalueHint') }}</p>
    </form>

    <template #footer>
      <Button variant="ghost" :disabled="reset.isPending.value" @click="reset.mutate()">{{ t('asset.revalueReset') }}</Button>
      <Button form="asset-revalue-form" type="submit" :disabled="save.isPending.value">
        {{ t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
