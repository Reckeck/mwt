<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'

import { LOYALTY_UNITS } from '../loyalty-edit.constants'
import type { LoyaltyInfoState } from '../model/use-loyalty-info'
import type { ProgramField } from '../loyalty-edit.types'

/** Step 1 of `New programme` (Ops-2 · 13): what it is, where it lives, what a point is worth. */
const props = defineProps<{ state: LoyaltyInfoState; error: (field: ProgramField) => string | undefined }>()
const name = defineModel<string>('name', { required: true })
const wallet = defineModel<string>('wallet', { required: true })
const { t } = useI18n()
</script>

<template>
  <div class="@container grid gap-4">
    <FormField for="program-name" :label="t('bonuses.name')" required :error="props.error('name')">
      <Input id="program-name" v-model="name" maxlength="64" :placeholder="t('bonuses.namePlaceholder')" />
    </FormField>
    <div class="grid gap-4 @sm:grid-cols-2">
      <FormField for="program-wallet" :label="t('bonuses.wallet')" required :error="props.error('wallet')">
        <StoreNodeSelect
          id="program-wallet"
          v-model="wallet"
          :nodes="props.state.cards.value"
          flat
          :placeholder="t('bonuses.pickWallet')"
          :invalid="Boolean(props.error('wallet'))"
        />
      </FormField>
      <FormField for="program-unit" :label="t('bonuses.unit')" required>
        <Select id="program-unit" v-model="props.state.unit.value" :aria-label="t('bonuses.unit')">
          <SelectItem v-for="unit in LOYALTY_UNITS" :key="unit" :value="unit">{{ t(`loyaltyUnit.${unit}`) }}</SelectItem>
        </Select>
      </FormField>
    </div>
    <FormField
      for="loyalty-value"
      :label="t('bonuses.rate')"
      :error="props.error('value') ?? props.error('currency')"
    >
      <AmountInput id="loyalty-value" v-model="props.state.value.value" :decimals="4">
        <template #prefix>{{ t(`loyaltyUnit.one.${props.state.unit.value}`) }} =</template>
          <template #suffix>
            <CurrencyPicker
              id="loyalty-currency"
              v-model="props.state.currency.value"
              label=""
              chip
              :kinds="['fiat']"
              :error="props.error('currency')"
            />
          </template>
      </AmountInput>
      <p class="text-xs text-muted-foreground">{{ t('bonuses.rateHint') }}</p>
    </FormField>
  </div>
</template>
