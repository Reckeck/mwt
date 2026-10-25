<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import type { FormValidation } from '@/shared/lib/form'
import { AdvancedFields } from '@/shared/ui/advanced-fields'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { BUDGET_PERIODS, BUDGET_TARGETS, type BudgetTargetKind } from '../budget-edit.constants'
import type { BudgetField, BudgetSpending } from '../budget-edit.types'

/** Step 1 (Money 1 · 3): what the limit watches and per what period. */
const props = defineProps<{ form: FormValidation<BudgetField>; nodes: StoreTreeNode[]; spending: BudgetSpending | null }>()
const kind = defineModel<BudgetTargetKind>('kind', { required: true })
const storeId = defineModel<string>('storeId', { required: true })
const period = defineModel<BudgetPeriod>('period', { required: true })

const { t } = useI18n()
</script>

<template>
  <div class="grid gap-4">
    <FormField v-if="kind === 'category'" for="budget-category" :label="t('budget.on')" required :error="props.form.error('category')">
      <!-- The dialog puts the category picker here: a feature takes another feature only at its root. -->
      <slot name="category" />
      <p v-if="props.spending" class="text-xs text-muted-foreground">
        <i18n-t keypath="budget.fromSpending">
          <template #spent><Money :amount="props.spending.spent" :currency="props.spending.currency" :decimals="0" /></template>
          <template #when>{{ props.spending.when }}</template>
        </i18n-t>
      </p>
    </FormField>
    <FormField v-else-if="kind === 'store'" for="budget-store" :label="t('budget.on')" required :error="props.form.error('store')">
      <StoreNodeSelect id="budget-store" v-model="storeId" :nodes="props.nodes" :empty-label="t('budget.pickStore')" />
    </FormField>

    <div class="grid gap-1.5">
      <Label>{{ t('budget.period') }}</Label>
      <ToggleGroup
        type="single"
        variant="outline"
        class="justify-start"
        :model-value="period"
        @update:model-value="(value) => value && (period = value as BudgetPeriod)"
      >
        <ToggleGroupItem v-for="entry in BUDGET_PERIODS" :key="entry" :value="entry" class="max-md:h-10">
          <span class="capitalize">{{ t(`budget.periodKind.${entry}`) }}</span>
        </ToggleGroupItem>
      </ToggleGroup>
    </div>

    <AdvancedFields :label="t('budget.otherTarget')" :default-open="kind !== 'category'">
      <div class="grid gap-1.5">
        <Label for="budget-target">{{ t('budget.target') }}</Label>
        <Select id="budget-target" v-model="kind">
          <SelectItem v-for="entry in BUDGET_TARGETS" :key="entry" :value="entry">{{ t(`budget.targetKind.${entry}`) }}</SelectItem>
        </Select>
      </div>
    </AdvancedFields>
  </div>
</template>
