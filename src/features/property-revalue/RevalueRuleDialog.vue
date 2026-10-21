<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useFormValidation } from '@/shared/lib/form'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Select, SelectItem } from '@/shared/ui/select'

import { draftOf, needsFloor, type RuleField, ruleErrors, ruleOf } from './model/rule-draft'
import type { RevalueRuleDialogProps } from './property-revalue.types'

/** How a property's value moves after its last valuation: off, with inflation, or a fixed percent a year. */
const props = defineProps<RevalueRuleDialogProps>()
const open = defineModel<boolean>('open', { default: false })
const { t } = useI18n()
const queryClient = useQueryClient()

const draft = ref(draftOf(props.rule))
watch(open, (isOpen) => {
  if (isOpen) draft.value = draftOf(props.rule)
})

const form = useFormValidation<RuleField>(() => ruleErrors(draft.value), { resetOn: open })
const floorNeeded = computed(() => needsFloor(draft.value))
const floorShown = computed(() => draft.value.mode === 'fixed')

const save = useMutation({
  mutationFn: () => call('property_set_revalue', { storeId: props.storeId, rule: ruleOf(draft.value) }),
  onSuccess: () => {
    for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS, ['property-valuations']])
      void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'revalueRule.saveFailed', success: 'revalueRule.saved' },
})
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="t('revalueRule.title')"
    :description="t('revalueRule.description')"
    form-id="revalue-rule-form"
    size="sm"
    :pending="save.isPending.value"
    @submit="form.submit(save.mutate, $event)"
  >
    <FormField for="revalue-mode" :label="t('revalueRule.mode')">
      <Select id="revalue-mode" v-model="draft.mode">
        <SelectItem v-for="mode in ['off', 'inflation', 'fixed']" :key="mode" :value="mode">
          {{ t(`revalueRule.modes.${mode}`) }}
        </SelectItem>
      </Select>
    </FormField>
    <FormField
      v-if="draft.mode === 'fixed'"
      for="revalue-pct"
      :label="t('revalueRule.pct')"
      required
      :error="form.error('pct')"
      :hint="t('revalueRule.pctHint')"
    >
      <AmountInput id="revalue-pct" v-model="draft.pct" allow-negative :decimals="2" />
    </FormField>
    <div v-if="floorShown" class="grid gap-3 @mid:grid-cols-[1fr_10rem]">
      <FormField for="revalue-floor" :label="t('revalueRule.floor')" :required="floorNeeded" :error="form.error('floor')">
        <AmountInput id="revalue-floor" v-model="draft.floor" :decimals="2" />
      </FormField>
      <FormField for="revalue-floor-kind" :label="t('revalueRule.floorKind')">
        <Select id="revalue-floor-kind" v-model="draft.floorKind">
          <SelectItem value="pct_of_purchase">{{ t('revalueRule.floorKinds.pct') }}</SelectItem>
          <SelectItem value="amount">{{ props.currency.toUpperCase() }}</SelectItem>
        </Select>
      </FormField>
    </div>
    <p class="text-xs text-muted-foreground">{{ t(`revalueRule.hint.${draft.mode}`) }}</p>
  </FormDialog>
</template>
