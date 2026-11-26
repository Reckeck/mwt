<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import type { FormValidation } from '@/shared/lib/form'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import type { GoalField } from '../goal-edit.types'

/** What is saved for — a name and how much. */
const props = defineProps<{ form: FormValidation<GoalField> }>()
const name = defineModel<string>('name', { required: true })
const target = defineModel<string>('target', { required: true })
const currency = defineModel<string>('currency', { required: true })

const { t } = useI18n()
</script>

<template>
  <div class="grid gap-4">
    <FormField for="goal-name" :label="t('goal.name')" required :error="props.form.error('name')">
      <Input id="goal-name" v-model="name" :placeholder="t('goal.namePlaceholder')" />
    </FormField>
    <FormField for="goal-target" :label="t('goal.howMuch')" required :error="props.form.error('target') ?? props.form.error('currency')">
      <AmountInput id="goal-target" v-model="target">
        <template #suffix>
          <CurrencyPicker id="goal-currency" v-model="currency" label="" chip />
        </template>
      </AmountInput>
    </FormField>
  </div>
</template>
