<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { useFormValidation } from '@/shared/lib/form'
import { nonNegativeAmount, required, firstError } from '@/shared/lib/form'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'

/**
 * «Ручной итог»: a person's total fixed by hand (the bill of a guest who ate only salad); what is
 * left of the spent sum is shared among the others. The backend refuses totals above what was spent.
 */
const props = defineProps<{ person: PersonView | null; current: string; currency: string; spent: string; pending?: boolean }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ save: [total: string] }>()

const { t } = useI18n()
const total = ref('')
watch(open, (now) => {
  if (now) total.value = props.current || props.person?.share || ''
})
const form = useFormValidation(() => ({ total: firstError(required(total.value), nonNegativeAmount(total.value)) }), { resetOn: open })
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-override"
    form-id="shared-override-form"
    size="sm"
    :title="t('sharedEvents.override.title', { name: props.person?.name || t('sharedEvents.me') })"
    :description="t('sharedEvents.override.description')"
    :submit-label="t('actions.save')"
    :pending="props.pending"
    @submit="form.submit(() => emit('save', total), $event)"
  >
    <FormField for="override-total" :label="t('sharedEvents.override.total')" required :error="form.error('total')">
      <AmountInput id="override-total" v-model="total" :decimals="2" />
      <template #hint>
        {{ t('sharedEvents.override.spent') }}
        <Money :amount="props.spent" :currency="props.currency" :convert="false" class="font-medium text-foreground" />
      </template>
    </FormField>
  </FormDialog>
</template>
