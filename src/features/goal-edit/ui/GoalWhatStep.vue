<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import type { FormValidation } from '@/shared/lib/form'
import { AmountInput } from '@/shared/ui/amount-input'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import type { GoalField } from '../goal-edit.types'

/** Step 1 (Money 1 · 7): what is saved for — a name, how much, by when. */
const props = defineProps<{ form: FormValidation<GoalField> }>()
const name = defineModel<string>('name', { required: true })
const target = defineModel<string>('target', { required: true })
const currency = defineModel<string>('currency', { required: true })
const deadline = defineModel<string>('deadline', { required: true })

const { t } = useI18n()
</script>

<template>
  <div class="grid gap-4">
    <FormField for="goal-name" :label="t('goal.name')" required :error="props.form.error('name')">
      <Input id="goal-name" v-model="name" :placeholder="t('goal.namePlaceholder')" />
    </FormField>
    <div class="grid items-start gap-4 sm:grid-cols-2">
    <FormField for="goal-target" :label="t('goal.howMuch')" required :error="props.form.error('target') ?? props.form.error('currency')">
      <AmountInput id="goal-target" v-model="target">
        <template #suffix>
          <CurrencyPicker id="goal-currency" v-model="currency" label="" chip />
        </template>
      </AmountInput>
    </FormField>
    <FormField for="goal-deadline" :label="t('goal.deadline')" :error="props.form.error('deadline')">
      <DateTimePicker id="goal-deadline" v-model="deadline" :placeholder="t('goal.noDeadline')" />
    </FormField>
    </div>
  </div>
</template>
