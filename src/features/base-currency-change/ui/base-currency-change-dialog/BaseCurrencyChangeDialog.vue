<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ArrowRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { Badge } from '@/shared/ui/badge'
import { FormDialog } from '@/shared/ui/form-dialog'
import { Separator } from '@/shared/ui/separator'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { useBaseCurrencyChange } from '../../model/use-base-currency-change'

/**
 * Asked before the base currency changes (Currency-2 · 9): «USD → RUB», two
 * lists — what is recounted and what is not — and the budget's currency.
 */
const props = defineProps<{ target: string }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()

const { from, to, budget, askBudget, budgetChoice, example, save } = useBaseCurrencyChange(() => props.target, open)
const pickBudget = (value: unknown) => {
  if (value === 'keep' || value === 'follow') budgetChoice.value = value
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="t('baseChange.title')"
    size="confirm"
    form-id="base-currency-change-form"
    :submit-label="t('baseChange.submit', { code: to.toUpperCase() })"
    :pending="save.isPending.value"
    @submit="save.mutate()"
  >
    <div class="flex items-center gap-2.5" data-base-change>
      <Badge variant="secondary" class="h-7 text-sm font-normal">{{ from.toUpperCase() }}</Badge>
      <ArrowRight class="size-4 text-muted-foreground" aria-hidden="true" />
      <Badge variant="outline" class="h-7 border-primary text-sm font-normal text-primary">{{ to.toUpperCase() }}</Badge>
    </div>
    <div class="flex flex-col gap-1.5 text-sm">
      <b class="font-semibold">{{ t('baseChange.recounted') }}</b>
      <span v-if="example">
        ·
        <i18n-t keypath="baseChange.worthExample" tag="span">
          <template #example>
            <Money :amount="example.before.amount" :currency="example.before.currency" :decimals="2" /> →
            <Money :amount="example.after.amount" :currency="example.after.currency" :decimals="0" />
          </template>
        </i18n-t>
      </span>
      <span v-else>· {{ t('baseChange.worth') }}</span>
      <span>· {{ t('baseChange.history') }}</span>
    </div>
    <div class="flex flex-col gap-1.5 text-sm">
      <b class="font-semibold">{{ t('baseChange.kept') }}</b>
      <span>· {{ t('baseChange.amounts') }}</span>
      <span>· {{ t('baseChange.limits') }}</span>
    </div>
    <template v-if="askBudget">
      <Separator />
      <div class="flex flex-wrap items-center gap-3 text-sm" data-base-change-budget>
        <span class="min-w-0 flex-1">{{ t('baseChange.budgetNow', { code: budget.toUpperCase() }) }}</span>
        <ToggleGroup :model-value="budgetChoice" type="single" variant="segmented" :spacing="1" @update:model-value="pickBudget">
          <ToggleGroupItem value="keep" class="px-2.5">{{ t('baseChange.keep') }}</ToggleGroupItem>
          <ToggleGroupItem value="follow" class="px-2.5">{{ t('baseChange.follow', { code: to.toUpperCase() }) }}</ToggleGroupItem>
        </ToggleGroup>
      </div>
    </template>
  </FormDialog>
</template>
