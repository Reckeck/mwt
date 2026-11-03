<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { CurrencyPicker, Money } from '@/entities/currency'
import { AssetInput, resolveTicker } from '@/entities/asset'
import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { AmountInput } from '@/shared/ui/amount-input'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'
import { SyncChangedBanner } from '@/shared/ui/sync-changed-banner'
import { positiveAmount, required, useEditBase, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'

import type { PriceAlertDialogProps } from './price-alert-edit.types'
import { alertInputOf, alertInputOfFields, fieldsOfAlert, type AlertFields } from './lib/alert-fields'
import { useCurrentPrice } from './model/use-current-price'

/**
 * `New price alert` (Auto-2 · 11): asset and condition side by side, the
 * level with its currency inside the field and today's price under it, and
 * whether it fires once or on every crossing.
 */
const props = withDefaults(defineProps<PriceAlertDialogProps>(), { alert: null, storeId: null, preset: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const asset = ref<AssetRef | null>(null)
const condition = ref<AlertCondition>('above')
const price = ref('')
const currency = ref('usd')
const once = ref(true)

/** Writes fields into the form; `Refresh` and opening both go through it. */
function load(fields: AlertFields) {
  asset.value = fields.asset
  condition.value = fields.condition
  price.value = fields.price
  currency.value = fields.currency
  once.value = fields.once
}
const currentFields = (): AlertFields => ({
  asset: asset.value,
  condition: condition.value,
  price: price.value,
  currency: currency.value,
  once: once.value,
})

watch(open, async (isOpen) => {
  if (!isOpen) return
  const preset = props.alert ? null : props.preset
  load(
    props.alert
      ? fieldsOfAlert(props.alert)
      : { asset: null, condition: preset?.condition ?? 'above', price: preset?.price ?? '', currency: 'usd', once: true },
  )
  if (preset) asset.value = await resolveTicker(preset.symbol)
}, { immediate: true })

/** The alert as the sync keeps it now (`['price-alerts']` is re-read after `vault-synced`); the prop may be a stale copy. */
const alerts = useQuery({ queryKey: ['price-alerts'], queryFn: () => call<PriceAlert[]>('list_price_alerts'), enabled: open })
const edit = useEditBase({
  open,
  entity: () => props.alert,
  live: () => (props.alert ? alerts.data.value?.find((entry) => entry.id === props.alert?.id) : undefined),
  model: (entity: PriceAlert) => fieldsOfAlert(entity),
})
function refreshFromOtherDevice() {
  edit.refresh(currentFields, load)
}

const current = useCurrentPrice(asset, currency)

const form = useFormValidation(
  () => ({
    asset: required(asset.value, 'pick'),
    price: positiveAmount(price.value),
    currency: required(currency.value, 'pick'),
  }),
  { resetOn: open },
)

const save = useMutation({
  mutationFn: () => {
    // Switched off from `Mine`; the dialog keeps it as it was.
    const input = alertInputOfFields(currentFields(), {
      active: props.alert?.active ?? true,
      storeId: (props.alert ? props.alert.store_id : props.storeId) ?? undefined,
    })
    return props.alert
      ? call<PriceAlert>('update_price_alert', {
          input: { id: props.alert.id, alert: input, base: edit.base.value && alertInputOf(edit.base.value) },
        })
      : call<PriceAlert>('create_price_alert', { input })
  },
  onSuccess: () => {
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'alert.saveFailed', success: () => (props.alert ? t('alert.saved') : t('alert.created')) },
})
</script>

<template>
  <Dialog overlay="price-alert" v-model:open="open" :title="props.alert ? t('alert.edit') : t('alert.newPrice')" size="form">
    <form id="price-alert-form" class="grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <SyncChangedBanner v-if="edit.changedElsewhere.value" @refresh="refreshFromOtherDevice" />
      <div class="grid items-start gap-4 sm:grid-cols-2">
        <AssetInput v-model="asset" :label="t('alert.asset')" required :error="form.error('asset')" />
        <FormField for="alert-condition" :label="t('alert.condition')" required>
          <Select id="alert-condition" v-model="condition">
            <SelectItem value="above">{{ t('alert.priceAbove') }}</SelectItem>
            <SelectItem value="below">{{ t('alert.priceBelow') }}</SelectItem>
          </Select>
        </FormField>
      </div>
      <FormField
        for="alert-price"
        :label="t('alert.value')"
        required
        :error="form.error('price') ?? form.error('currency')"
      >
        <template v-if="current !== null" #hint>
          <i18n-t keypath="alert.now">
            <template #price><Money :amount="current" :currency="currency" /></template>
          </i18n-t>
        </template>
        <AmountInput id="alert-price" v-model="price" size="lg">
          <template #suffix>
            <CurrencyPicker id="alert-currency" v-model="currency" label="" chip />
          </template>
        </AmountInput>
      </FormField>
      <SwitchRow id="alert-once" v-model="once" :label="t('alert.once')" />
    </form>
    <template #footer>
      <Button variant="ghost" class="max-md:h-11" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button class="max-md:h-11 max-md:flex-1" form="price-alert-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
