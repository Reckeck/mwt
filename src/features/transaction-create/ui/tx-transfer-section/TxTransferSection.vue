<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ArrowUpDown } from 'lucide-vue-next'

import { AssetInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'

import { useTxForm } from '../../model/use-tx-form'
import { AccountRefPicker } from '../account-ref-picker'

/** Transfer: one asset between two places, possibly a debt. */
const { t } = useI18n()
const { form, check, swapSides, debts, debtValue } = useTxForm()
</script>

<template>
  <div class="grid gap-3 @sm:grid-cols-2">
    <AssetInput v-model="form.asset" required :error="check.error('asset')" :label="t('form.asset')" />
    <FormField for="tx-amount" :label="t('form.amount')" required :error="check.error('amount')">
      <AmountInput id="tx-amount" v-model="form.amount" :placeholder="t('form.amountHint')" />
    </FormField>
  </div>
  <div class="grid items-end gap-3 @sm:grid-cols-[1fr_auto_1fr]">
    <FormField :label="t('tx.field.from')" required :error="check.error('from')">
      <AccountRefPicker v-model="form.from" />
    </FormField>
    <Button type="button" variant="ghost" size="icon" :title="t('tx.field.swap')" :aria-label="t('tx.field.swap')" @click="swapSides">
      <ArrowUpDown class="size-4" />
    </Button>
    <FormField :label="t('tx.field.to')" required :error="check.error('to')">
      <AccountRefPicker v-model="form.to" />
    </FormField>
  </div>
  <div v-if="debts.length" class="grid gap-1.5">
    <Label for="tx-debt">{{ t('debt.title') }}</Label>
    <Select id="tx-debt" v-model="debtValue">
      <SelectItem value="">{{ t('debt.none') }}</SelectItem>
      <SelectItem v-for="effect in debts" :key="effect" :value="effect">{{ t(`debt.${effect}`) }}</SelectItem>
    </Select>
    <p class="text-xs text-muted-foreground">{{ t('debt.hint') }}</p>
  </div>
</template>
