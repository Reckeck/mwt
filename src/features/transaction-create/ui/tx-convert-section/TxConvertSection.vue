<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ArrowUpDown } from 'lucide-vue-next'

import { AssetInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { formatQuantity } from '@/shared/lib/format'

import { useTxForm } from '../../model/use-tx-form'
import { AccountRefPicker } from '../account-ref-picker'
import { SplitRows } from '../split-rows'

/** Convert: give one asset, receive another — straight or through someone (P2P), to one account or several. */
const { t } = useI18n()
const { form, check, props, swapSides, pickConvertMode, receivedTotal, toAccount, impliedRate } = useTxForm()
</script>

<template>
  <ToggleGroup
    v-if="!props.template"
    type="single"
    variant="outline"
    size="sm"
    class="w-fit"
    :spacing="1"
    :model-value="form.convertMode"
    :aria-label="t('tx.p2p.mode')"
    data-tour="tx-convert-mode"
    @update:model-value="pickConvertMode"
  >
    <ToggleGroupItem value="direct">{{ t('tx.p2p.direct') }}</ToggleGroupItem>
    <ToggleGroupItem value="p2p">{{ t('tx.p2p.through') }}</ToggleGroupItem>
  </ToggleGroup>
  <p v-if="form.convertMode === 'p2p'" class="text-xs text-muted-foreground">{{ t('tx.p2p.hint') }}</p>

  <div class="grid gap-3 @sm:grid-cols-3">
    <FormField :label="form.convertMode === 'p2p' ? t('tx.p2p.from') : t('tx.field.give')" required :error="check.error('from')">
      <AccountRefPicker v-model="form.from" :contacts="form.convertMode !== 'p2p'" :raw="form.convertMode !== 'p2p'" />
    </FormField>
    <AssetInput v-model="form.fromAsset" required :error="check.error('fromAsset')" :label="t('form.asset')" />
    <FormField for="tx-from-amount" :label="t('form.amount')" required :error="check.error('fromAmount')">
      <AmountInput id="tx-from-amount" v-model="form.fromAmount" />
    </FormField>
  </div>
  <FormField v-if="form.convertMode === 'p2p'" :label="t('tx.p2p.via')" required :error="check.error('via')">
    <AccountRefPicker v-model="form.via" :stores="false" />
  </FormField>
  <div class="flex justify-center">
    <Button type="button" variant="ghost" size="icon" :title="t('tx.field.swap')" :aria-label="t('tx.field.swap')" @click="swapSides">
      <ArrowUpDown class="size-4" />
    </Button>
  </div>
  <!-- What arrives, and the accounts it lands on: the counterparty may pay
       out to several of them; each row is an account and its amount. -->
  <div class="grid gap-3 @sm:grid-cols-[minmax(0,1fr)_14rem]">
    <div class="grid gap-1.5">
      <Label required>{{ form.convertMode === 'p2p' ? t('tx.p2p.receiveTo') : t('tx.field.receive') }}</Label>
      <p class="text-xs text-muted-foreground">{{ t('tx.split.receiveHint') }}</p>
    </div>
    <AssetInput v-model="form.toAsset" required :error="check.error('toAsset')" :label="t('tx.split.receiveAsset')" />
  </div>
  <div class="grid gap-2" data-tour="tx-split">
    <div class="grid items-start gap-2 @sm:grid-cols-[minmax(0,1fr)_10rem_2rem]">
      <FormField :error="check.error('to')">
        <AccountRefPicker v-model="form.to" :contacts="form.convertMode !== 'p2p'" :raw="form.convertMode !== 'p2p'" />
      </FormField>
      <FormField :error="check.error('toAmount')">
        <AmountInput id="tx-to-amount" v-model="form.toAmount" :placeholder="t('form.amount')" :aria-label="t('form.amount')" />
      </FormField>
    </div>
    <FormField v-if="!props.template" :error="check.error('extra')">
      <SplitRows v-model="form.extra" kind="convert" :mode="form.convertMode" />
    </FormField>
    <p v-if="form.extra.length && form.toAsset" class="text-xs text-muted-foreground">
      {{ t('tx.split.receivedTotal', { total: `${formatQuantity(receivedTotal)} ${form.toAsset.symbol}` }) }}
    </p>
  </div>
  <p v-if="toAccount.moneyOnly.value && form.toAsset" class="text-xs text-muted-foreground">
    {{ t('tx.toCurrencyHint', { code: form.toAsset.symbol }) }}
  </p>
  <div class="grid gap-1.5">
    <Label for="tx-rate">{{ t('tx.field.rate') }}</Label>
    <AmountInput id="tx-rate" v-model="form.rate" :placeholder="impliedRate" />
    <p class="text-xs text-muted-foreground">{{ impliedRate || t('tx.field.rateHint') }}</p>
  </div>
</template>
