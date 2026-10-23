<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { AmountInput } from '@/shared/ui/amount-input'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'

import { LOYALTY_UNITS } from '../loyalty-edit.constants'

import type { LoyaltyField, LoyaltyInfoState } from '../model/use-loyalty-info'

/** Points, their worth, what burns next, how the programme knows the user and which cards earn. */
const props = defineProps<{ state: LoyaltyInfoState; error: (field: LoyaltyField) => string | undefined }>()
const { t } = useI18n()
</script>

<template>
  <div class="@container grid gap-4">
    <div class="grid gap-4 @xl:grid-cols-3">
      <FormField for="loyalty-symbol" :label="t('loyalty.symbol')" required :error="props.error('symbol')">
        <!-- The name of a programme's points, not a listed asset. -->
        <Input id="loyalty-symbol" v-model="props.state.symbol.value" class="uppercase" maxlength="16" data-picker-ok />
        <p class="text-xs text-muted-foreground">{{ t('loyalty.symbolHint') }}</p>
      </FormField>
      <FormField for="loyalty-value" :label="t('loyalty.pointValue')" required :error="props.error('value')">
        <AmountInput id="loyalty-value" v-model="props.state.value.value" :decimals="4" />
      </FormField>
      <CurrencyPicker
        v-model="props.state.currency.value"
        :label="t('form.currency')"
        :kinds="['fiat']"
        required
        :error="props.error('currency')"
      />
    </div>
    <div class="grid gap-4 @xl:grid-cols-2">
      <div class="grid gap-1.5">
        <Label for="loyalty-login">{{ t('loyalty.login') }}</Label>
        <Input id="loyalty-login" v-model="props.state.login.value" maxlength="64" />
        <p class="text-xs text-muted-foreground">{{ t('loyalty.loginHint') }}</p>
      </div>
      <div class="grid gap-1.5">
        <Label for="loyalty-sources">{{ t('loyalty.sources') }}</Label>
        <StoreNodeSelect
          id="loyalty-sources"
          v-model:ids="props.state.sources.value"
          :nodes="props.state.cards.value"
          flat
          multiple
          :empty-label="t('loyalty.noSource')"
        />
        <p class="text-xs text-muted-foreground">{{ t('loyalty.sourcesHint') }}</p>
      </div>
    </div>
    <div class="grid gap-4 @xl:grid-cols-3">
      <div class="grid gap-1.5">
        <Label for="loyalty-expires">{{ t('loyalty.expiresAt') }}</Label>
        <DateTimePicker id="loyalty-expires" v-model="props.state.expiresAt.value" />
      </div>
      <FormField for="loyalty-expiring" :label="t('loyalty.expiringPoints')" :error="props.error('expiring')">
        <AmountInput id="loyalty-expiring" v-model="props.state.expiring.value" />
      </FormField>
      <FormField for="loyalty-unit" :label="t('bonuses.unit')">
        <Select id="loyalty-unit" v-model="props.state.unit.value" :aria-label="t('bonuses.unit')">
          <SelectItem v-for="unit in LOYALTY_UNITS" :key="unit" :value="unit">{{ t(`loyaltyUnit.${unit}`) }}</SelectItem>
        </Select>
      </FormField>
    </div>
  </div>
</template>
