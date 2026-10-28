<script setup lang="ts">
import { Button } from '@/shared/ui/button'
import { computed, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'

import { TxCategorySelect } from '../tx-category-select'
import { AssetChip } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'

import { useFrequentCategory } from '../../model/use-frequent-category'
import { useTxForm } from '../../model/use-tx-form'
import { categoryGroupsFor } from '../../transaction-form.model'
import { AvailableHint } from '../available-hint'

/**
 * Payment and top-up in the short form (OpForm-1 · 1, OpForm-2 · 8): the
 * amount large with its currency as a chip, the wallet (from the dialog,
 * `#store`) next to the category, which offers the usual one under it.
 */
defineSlots<{ store?: () => unknown }>()

const { t } = useI18n()
const { form, check, props, needsCategory, noteExpression } = useTxForm()
const categories = useCategories()

/** One category here; more than one stays possible in the full editor. */
const category = computed({
  get: () => form.categories[0] ?? '',
  set: (key: string | string[]) => {
    const picked = Array.isArray(key) ? key[0] : key
    form.categories = picked ? [picked] : []
  },
})
const frequent = useFrequentCategory(() => form.kind)
const frequentLabel = computed(() =>
  frequent.value ? categoryKeyLabel(frequent.value, categories.data.value, t) : '',
)
/** Canvas wording (OpForm-1 · 3): one plain sentence per field, not the generic rule text. */
/** The hint disappears once used; the focus goes on to the comment, not to the page, so Enter still saves. */
function pickFrequent() {
  category.value = frequent.value ?? ''
  void nextTick(() => document.getElementById('tx-comment')?.focus())
}

const amountError = computed(() => (check.error('amount') ? t('form.amountError') : check.error('asset')))
const categoryError = computed(() =>
  check.error('categories') ? t(form.kind === 'topup' ? 'form.sourceError' : 'form.categoryError') : undefined,
)
</script>

<template>
  <FormField for="tx-amount" :label="t('form.sum')" required :error="amountError">
    <AmountInput id="tx-amount" @expression="noteExpression('tx-amount', $event)" v-model="form.amount" size="lg" :placeholder="t('form.amountZero')">
      <template #suffix>
        <AssetChip v-model="form.asset" :label="t('form.asset')" :invalid="Boolean(check.error('asset'))" />
      </template>
    </AmountInput>
    <AvailableHint
      v-if="form.kind === 'payment'"
      :store-id="props.storeId"
      :asset="form.asset"
      @max="(quantity) => (form.amount = quantity)"
    />
  </FormField>

  <div class="grid items-start gap-3 @md:grid-cols-2">
    <slot name="store" />
    <FormField
      for="tx-categories"
      :label="form.kind === 'topup' ? t('tx.field.source') : t('form.category')"
      :required="needsCategory"
      :error="categoryError"
    >
      <TxCategorySelect
        id="tx-categories"
        v-model="category"
        :groups="categoryGroupsFor(form.kind)"
        :placeholder="t('form.categoryPlaceholder')"
      />
      <p v-if="!categoryError" class="text-xs text-muted-foreground">
        <template v-if="form.kind === 'topup'">{{ t('tx.field.sourceHint') }}</template>
        <Button
          v-else-if="frequentLabel && !category"
          type="button"
          variant="link"
          size="sm"
          class="h-auto p-0 text-xs font-normal text-muted-foreground hover:text-foreground"
          data-frequent-category
          @click="pickFrequent"
        >
          {{ t('form.frequentCategory', { name: frequentLabel }) }}
        </Button>
      </p>
    </FormField>
  </div>
</template>
