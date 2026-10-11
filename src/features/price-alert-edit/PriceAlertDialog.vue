<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { CurrencyPicker } from '@/entities/currency'
import { AssetInput } from '@/entities/asset'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { AmountInput } from '@/shared/ui/amount-input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call, parseError } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { PriceAlertDialogProps } from './price-alert-edit.types'

const props = withDefaults(defineProps<PriceAlertDialogProps>(), { alert: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const asset = ref<AssetRef | null>(null)
const condition = ref<AlertCondition>('above')
const price = ref('')
const currency = ref('usd')
const active = ref(true)

watch(open, (isOpen) => {
  if (!isOpen) return
  asset.value = props.alert?.asset ?? null
  condition.value = props.alert?.condition ?? 'above'
  price.value = props.alert?.price ?? ''
  currency.value = props.alert?.currency ?? 'usd'
  active.value = props.alert?.active ?? true
})

const valid = computed(() => asset.value !== null && Number(price.value) > 0)

const save = useMutation({
  mutationFn: () => {
    const picked = asset.value as AssetRef
    const input: PriceAlertInput = {
      asset: {
        symbol: picked.symbol,
        name: picked.name,
        coingecko_id: picked.coingecko_id,
        chain: picked.chain,
        contract: picked.contract,
      },
      condition: condition.value,
      price: price.value.trim(),
      currency: currency.value,
      active: active.value,
    }
    return props.alert
      ? call<PriceAlert>('update_price_alert', { input: { id: props.alert.id, alert: input } })
      : call<PriceAlert>('create_price_alert', { input })
  },
  onSuccess: () => {
    toast.success(props.alert ? t('alert.saved') : t('alert.created'))
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  onError: (err) => toast.error(t('alert.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.alert ? t('alert.edit') : t('alert.new')">
    <form id="price-alert-form" class="@container grid gap-4" @submit.prevent="valid && save.mutate()">
      <AssetInput v-model="asset" :label="t('alert.asset')" />
      <div class="grid gap-3 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label for="alert-condition">{{ t('alert.condition') }}</Label>
          <Select id="alert-condition" v-model="condition">
            <option value="above">{{ t('alert.above') }}</option>
            <option value="below">{{ t('alert.below') }}</option>
          </Select>
        </div>
        <div class="grid gap-1.5">
          <Label for="alert-price">{{ t('alert.price') }}</Label>
          <AmountInput id="alert-price" v-model="price" />
        </div>
        <div class="grid gap-1.5">
          <Label for="alert-currency">{{ t('alert.currency') }}</Label>
          <CurrencyPicker id="alert-currency" v-model="currency" />
        </div>
      </div>
      <label class="flex items-center gap-2 text-sm">
        <input v-model="active" type="checkbox" class="size-4 accent-primary" />
        {{ t('alert.active') }}
      </label>
      <p class="text-xs text-muted-foreground">{{ t('alert.hint') }}</p>
    </form>
    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="price-alert-form" type="submit" :disabled="!valid || save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
