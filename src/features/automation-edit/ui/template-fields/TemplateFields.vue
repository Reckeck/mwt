<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { StoreNodeSelect } from '@/entities/store'
import { call } from '@/shared/api/ipc'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryOptions } from '@/shared/lib/categories'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import type { WizardDraft } from '../../automation-edit.types'
import {
  TEMPLATE_KINDS,
  type TemplateDraft,
  type TemplateKind,
  templateDraftOf,
  templateErrors,
  templateKindOf,
  templateStartOf,
  templateToInput,
} from '../../lib/template-input'

/**
 * The operation an automation makes, in plain words: what (expense, income, transfer), how much,
 * from which wallet, for which category. An operation of another kind (an exchange, a trade) is
 * kept as stored and shown as a line, so a save never rewrites it into something else.
 * Exposes `valid`, `input`, `draft` and `showErrors()`; the host saves.
 */
const props = withDefaults(
  defineProps<{ storeId: string; transaction?: Transaction | null; initial?: Partial<WizardDraft> | null; propertyId?: string }>(),
  { transaction: null, initial: null, propertyId: '' },
)
const emit = defineEmits<{ submit: [] }>()
const { t } = useI18n()
const settings = useSettingsStore()

const unsupported = computed(() => Boolean(props.transaction) && templateKindOf(props.transaction) === null)
const draft = reactive<TemplateDraft>(
  templateDraftOf(props.transaction, templateStartOf(props.initial)),
)
const shown = ref(false)

const tree = useStoreTree()
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))
const categories = useCategories()
const choices = computed(() =>
  categoryOptions(categories.data.value, t, draft.kind === 'topup' ? ['income'] : ['fixed', 'daily', 'savings']),
)
const overview = useQuery({
  // no placeholder: the asset is this wallet's own, not the last one's
  queryKey: computed(() => ['store', props.storeId, false]),
  queryFn: () => call<StoreOverview>('get_store_overview', { storeId: props.storeId, includeChildren: false }),
  enabled: computed(() => props.storeId !== ''),
})
/** The asset the wallet is bound to, else its largest position, else the base currency. */
const asset = computed<AssetRef | null>(() => {
  const data = overview.data.value
  const binding = data?.store.binding
  if (binding && typeof binding === 'object' && 'asset' in binding) return binding.asset
  return data?.positions[0]?.asset ?? null
})

const errors = computed(() => (shown.value ? templateErrors(draft, props.storeId) : {}))
const valid = computed(() => !unsupported.value && Object.keys(templateErrors(draft, props.storeId)).length === 0)
const input = computed<TransactionInput | null>(() => {
  if (unsupported.value) return null
  return templateToInput(draft, props.storeId, asset.value, settings.baseCurrency, props.propertyId)
})
const wizardDraft = computed<WizardDraft>(() => ({
  kind: draft.kind,
  categories: draft.category ? [draft.category] : [],
  amount: draft.amount,
  comment: draft.comment,
  to: draft.to,
}))

const pickKind = (kind: unknown) => {
  if (typeof kind === 'string' && kind !== draft.kind) {
    draft.kind = kind as TemplateKind
    draft.category = ''
  }
}
defineExpose({ valid, input, draft: wizardDraft, showErrors: () => (shown.value = true) })
</script>

<template>
  <div class="grid gap-3" data-automation-template>
    <p v-if="unsupported" class="rounded-md bg-muted px-3 py-2 text-sm text-muted-foreground">{{ t('automation.template.unsupported') }}</p>
    <template v-else>
      <ToggleGroup
        :model-value="draft.kind"
        type="single"
        variant="outline"
        size="sm"
        class="justify-start"
        :aria-label="t('automation.template.kind')"
        @update:model-value="pickKind"
      >
        <ToggleGroupItem v-for="kind in TEMPLATE_KINDS" :key="kind" :value="kind">{{ t(`tx.kindTab.${kind}`) }}</ToggleGroupItem>
      </ToggleGroup>
      <slot name="store" />
      <FormField for="automation-template-amount" :label="t('automation.template.amount')" required :error="errors.amount">
        <AmountInput id="automation-template-amount" v-model="draft.amount" @keydown.enter.prevent="emit('submit')" />
      </FormField>
      <FormField v-if="draft.kind === 'transfer'" for="automation-template-to" :label="t('automation.template.to')" required :error="errors.to">
        <StoreNodeSelect id="automation-template-to" v-model="draft.to" :nodes="nodes.filter((node) => node.store.id !== props.storeId)" class="w-full" />
      </FormField>
      <FormField v-else for="automation-template-category" :label="t('automation.template.category')" required :error="errors.category">
        <Select id="automation-template-category" v-model="draft.category">
          <SelectItem v-for="choice in choices" :key="choice.value" :value="choice.value">{{ choice.label }}</SelectItem>
        </Select>
      </FormField>
      <FormField for="automation-template-comment" :label="t('automation.template.comment')">
        <Input id="automation-template-comment" v-model="draft.comment" @keydown.enter.prevent="emit('submit')" />
      </FormField>
    </template>
  </div>
</template>
