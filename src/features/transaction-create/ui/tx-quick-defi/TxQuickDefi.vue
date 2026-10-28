<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Info } from 'lucide-vue-next'

import { AssetChip } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { useTxForm } from '../../model/use-tx-form'
import { QUICK_DEFI_ACTIONS } from '../../transaction-form.constants'
import { AvailableHint } from '../available-hint'

/**
 * DeFi in «Сделка · <биржа>» (OpForm-2 · 11): staking or a pool, where, and
 * how much. What it earns comes later as income operations of its own;
 * unstaking, claims and pool ids stay in the full editor.
 */
const { t } = useI18n()
const { form, check, props, contextStore, overAvailable, noteExpression } = useTxForm()

if (!QUICK_DEFI_ACTIONS.includes(form.defiAction)) form.defiAction = 'stake'
const protocolHint = computed(() => (contextStore.value ? `${contextStore.value.name} Earn` : ''))
const amountError = computed(() => {
  if (!check.error('amount')) return check.error('asset')
  return overAvailable.value || t('form.amountError')
})
</script>

<template>
  <div class="grid gap-1.5">
    <Label>{{ t('tx.field.action') }}</Label>
    <ToggleGroup
      type="single"
      variant="segmented"
      :spacing="1"
      class="justify-start"
      :model-value="form.defiAction"
      :aria-label="t('tx.field.action')"
      @update:model-value="(value) => value && (form.defiAction = value as DefiAction)"
    >
      <ToggleGroupItem v-for="action in QUICK_DEFI_ACTIONS" :key="action" :value="action" class="px-3" :data-defi-action="action">
        {{ t(`tx.quick.defi.${action}`) }}
      </ToggleGroupItem>
    </ToggleGroup>
  </div>

  <div class="grid items-start gap-3 @md:grid-cols-2">
    <FormField for="tx-protocol" :label="t('tx.field.protocol')">
      <Input id="tx-protocol" v-model="form.protocol" autocomplete="off" :placeholder="protocolHint" />
    </FormField>
    <FormField for="tx-amount" :label="t('form.sum')" required :error="amountError">
      <AmountInput id="tx-amount" @expression="noteExpression('tx-amount', $event)" v-model="form.amount" :placeholder="t('form.amountZero')">
        <template #suffix>
          <AssetChip v-model="form.asset" :label="t('form.asset')" :invalid="Boolean(check.error('asset'))" />
        </template>
      </AmountInput>
      <AvailableHint :store-id="props.storeId" :asset="form.asset" @max="(quantity) => (form.amount = quantity)" />
    </FormField>
  </div>

  <p class="flex items-center gap-2 text-sm text-muted-foreground" data-defi-note>
    <Info class="size-4 shrink-0" />{{ t('tx.quick.defiNote') }}
  </p>
</template>
