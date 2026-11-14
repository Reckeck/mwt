<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowUpDown } from 'lucide-vue-next'

import { AssetInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'

import { useTxForm } from '../../model/use-tx-form'
import { AccountRefPicker } from '../account-ref-picker'
import { ConvertVia } from '../convert-via'
import { SplitRows } from '../split-rows'
import { useConvertSplit } from '../../model/use-convert-split'
import { accountStoreId } from '@/shared/lib/transactions'
import { AvailableHint } from '../available-hint'

/** Convert: give one asset, receive another — optionally through an intermediary, to one account or several. */
const { t } = useI18n()
const { form, check, props, swapSides, toAccount, impliedRate, noteExpression } = useTxForm()
const fromStore = computed(() => accountStoreId(form.from))
const { split, remainder, distributedLine } = useConvertSplit()
</script>

<template>
  <div class="grid gap-3 @xl:grid-cols-3">
    <FormField :label="t('tx.field.give')" required :error="check.error('from')">
      <AccountRefPicker v-model="form.from" :contacts="!form.viaOpen" :raw="!form.viaOpen" />
    </FormField>
    <AssetInput v-model="form.fromAsset" required :error="check.error('fromAsset')" :label="t('form.asset')" />
    <FormField for="tx-from-amount" :label="t('form.amount')" required :error="check.error('fromAmount')">
      <AmountInput id="tx-from-amount" @expression="noteExpression('tx-from-amount', $event)" v-model="form.fromAmount" />
      <AvailableHint :store-id="fromStore" :asset="form.fromAsset" @max="(quantity) => (form.fromAmount = quantity)" />
    </FormField>
  </div>
  <ConvertVia v-if="!props.template" />
  <div class="flex justify-center">
    <Button type="button" variant="ghost" size="icon" :title="t('tx.field.swap')" :aria-label="t('tx.field.swap')" @click="swapSides">
      <ArrowUpDown class="size-4" />
    </Button>
  </div>
  <!-- What arrives: laid out like what leaves. The counterparty may pay out
       to several accounts; the extra rows go under it. -->
  <div class="grid gap-3 @xl:grid-cols-3">
    <FormField class="min-w-0" :label="t('tx.field.receive')" required :error="check.error('to')">
      <div class="flex items-start gap-2">
        <div class="min-w-0 flex-1">
          <AccountRefPicker v-model="form.to" :contacts="!form.viaOpen" :raw="!form.viaOpen" />
        </div>
        <AmountInput
          v-if="split"
          v-model="form.toShare"
          class="w-24 shrink-0"
          :placeholder="remainder"
          :aria-label="t('tx.split.accountN', { n: 1 })"
        />
      </div>
    </FormField>
    <AssetInput v-model="form.toAsset" required :error="check.error('toAsset')" :label="t('form.asset')" />
    <FormField for="tx-to-amount" :label="t('form.amount')" required :error="check.error('toAmount')">
      <AmountInput id="tx-to-amount" @expression="noteExpression('tx-to-amount', $event)" v-model="form.toAmount" />
    </FormField>
  </div>
  <div v-if="!props.template" class="grid gap-2">
    <FormField :error="check.error('extra')">
      <SplitRows v-model="form.extra" kind="convert" :via="form.viaOpen" />
    </FormField>
    <p v-if="split" class="text-xs" :class="check.error('shares') ? 'text-destructive' : 'text-muted-foreground'">
      {{ distributedLine }}
    </p>
  </div>
  <p v-if="toAccount.moneyOnly.value && form.toAsset" class="text-xs text-muted-foreground">
    {{ t('tx.toCurrencyHint', { code: form.toAsset.symbol }) }}
  </p>
  <div class="grid gap-1.5">
    <Label for="tx-rate">{{ t('tx.field.rate') }}</Label>
    <AmountInput id="tx-rate" @expression="noteExpression('tx-rate', $event)" v-model="form.rate" :placeholder="impliedRate" />
    <p class="text-xs text-muted-foreground">{{ impliedRate || t('tx.field.rateHint') }}</p>
  </div>
</template>
