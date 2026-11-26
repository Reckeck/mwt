<script setup lang="ts">
import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { useCategories } from '@/shared/api/queries'
import { categoryOptions } from '@/shared/lib/categories'
import type { FieldError } from '@/shared/lib/form'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { Select, SelectItem } from '@/shared/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { QUICK_KINDS, type QuickField, type QuickFormState, type QuickKind } from '../../lib/quick-form'
import { AccountRefPicker } from '../account-ref-picker'
import { QuickAmount } from '../quick-amount'

/**
 * The body of the quick add: the kind switch, the hero amount (keypad on a phone), the wallet,
 * the category and the date. It validates nothing itself: it shows what the owner passes in
 * `errors` (after a save was tried, rule 13).
 */
const props = defineProps<{ errors: Partial<Record<QuickField, FieldError | undefined>> }>()
const form = defineModel<QuickFormState>('form', { required: true })
const { t } = useI18n()

const categories = useCategories()
const GROUPS = { payment: ['daily', 'fixed', 'savings'], topup: ['income', 'savings'] } as const
const options = computed(() =>
  form.value.kind === 'transfer' ? [] : categoryOptions(categories.data.value, t, GROUPS[form.value.kind]),
)
// A category of the other kind does not carry over.
watch(options, (list) => {
  if (form.value.category && !list.some((option) => option.value === form.value.category)) form.value.category = ''
})

/** Clicking the lit segment would clear it; a kind always stays picked. */
function pickKind(value: unknown) {
  if (typeof value === 'string' && QUICK_KINDS.includes(value as QuickKind)) form.value.kind = value as QuickKind
}
</script>

<template>
  <div class="grid gap-4">
    <ToggleGroup type="single" variant="segmented" :spacing="1" class="w-full" data-quick-kind :model-value="form.kind" @update:model-value="pickKind">
      <ToggleGroupItem v-for="kind in QUICK_KINDS" :key="kind" :value="kind" class="flex-1" :data-kind="kind">
        {{ t(`quickAdd.kind.${kind}`) }}
      </ToggleGroupItem>
    </ToggleGroup>

    <FormField :error="props.errors.amount ?? undefined">
      <QuickAmount id="quick-amount" v-model="form.amount" :invalid="Boolean(props.errors.amount)" :aria-label="t('form.sum')" />
    </FormField>

    <FormField for="quick-store" :label="form.kind === 'transfer' ? t('tx.quick.fromWallet') : t('quickAdd.store')" required :error="props.errors.storeId ?? undefined">
      <AccountRefPicker id="quick-store" v-model="form.storeId" :invalid="Boolean(props.errors.storeId)" />
    </FormField>

    <FormField v-if="form.kind === 'transfer'" for="quick-to-store" :label="t('tx.quick.toWallet')" required :error="props.errors.toStoreId ?? undefined">
      <AccountRefPicker id="quick-to-store" v-model="form.toStoreId" :invalid="Boolean(props.errors.toStoreId)" />
    </FormField>
    <FormField v-else for="quick-category" :label="t('quickAdd.category')" required :error="props.errors.category ?? undefined">
      <Select id="quick-category" v-model="form.category" :invalid="Boolean(props.errors.category)" :placeholder="t('quickAdd.pickCategory')">
        <SelectItem v-for="option in options" :key="option.value" :value="option.value">{{ option.label }}</SelectItem>
      </Select>
    </FormField>

    <FormField for="quick-date" :label="t('quickAdd.date')" :error="props.errors.timestamp ?? undefined">
      <DateTimePicker id="quick-date" v-model="form.timestamp" mode="datetime" required :invalid="Boolean(props.errors.timestamp)" />
    </FormField>
  </div>
</template>
