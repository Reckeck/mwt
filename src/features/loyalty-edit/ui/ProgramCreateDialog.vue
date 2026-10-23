<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { WizardSteps } from '@/shared/ui/wizard-steps'
import { required, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'

import { invalidateLoyalty } from '../model/invalidate'
import { useLoyaltyInfo } from '../model/use-loyalty-info'
import { PROGRAM_COLOR } from '../loyalty-edit.constants'
import type { ProgramField } from '../loyalty-edit.types'
import ProgramRulesStep from './ProgramRulesStep.vue'
import ProgramWhatStep from './ProgramWhatStep.vue'

/**
 * «+ Программа» in two steps (Ops-2 · 13): what the programme is and what a
 * point is worth, then — all optional — how it earns. The ticker is made from
 * the name and can be changed on step 2.
 */
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ created: [store: Store] }>()
const { t } = useI18n()
const queryClient = useQueryClient()

const step = ref(1)
const name = ref('')
const wallet = ref('')
const contactId = ref('')
const state = useLoyaltyInfo()
/** The ticker last made from the name; a hand-typed one is kept. */
let madeSymbol = ''

watch(open, (value) => {
  if (!value) return
  step.value = 1
  name.value = ''
  wallet.value = ''
  contactId.value = ''
  state.load(null)
  madeSymbol = ''
})

const form = useFormValidation<ProgramField>(
  () => {
    const rules = state.rules()
    return step.value === 1
      ? { name: required(name.value), wallet: required(wallet.value, 'pick'), value: rules.value, currency: rules.currency }
      : { symbol: rules.symbol, expiring: rules.expiring }
  },
  { resetOn: [open, step] },
)

const steps = computed(() => [t('bonuses.step.what'), t('bonuses.step.how')])

const save = useMutation({
  mutationFn: () =>
    call<Store>('create_loyalty_program', {
      input: {
        name: name.value.trim(),
        color: PROGRAM_COLOR,
        contact_id: contactId.value || null,
        info: {
          ...state.toInfo(),
          source_store_ids: [wallet.value, ...state.sources.value.filter((id) => id !== wallet.value)],
        },
      } satisfies CreateLoyaltyProgramInput,
    }),
  onSuccess: (store) => {
    invalidateLoyalty(queryClient)
    open.value = false
    emit('created', store)
  },
  meta: {
    error: 'bonuses.createFailed',
    success: (data) => t('bonuses.createdNamed', { name: (data as Store).name }),
  },
})

function submit(event: Event) {
  if (step.value === 2) {
    form.submit(save.mutate, event)
    return
  }
  form.submit(() => {
    if (!state.symbol.value.trim() || state.symbol.value === madeSymbol) {
      madeSymbol = state.symbolFrom(name.value)
      state.symbol.value = madeSymbol
    }
    step.value = 2
  }, event)
}
</script>

<template>
  <Dialog v-model:open="open" :title="t('bonuses.new')">
    <form id="program-create-form" class="grid gap-4" data-tour="bonuses-wizard" @submit.prevent="submit">
      <WizardSteps :step="step" :titles="steps" />
      <ProgramWhatStep v-if="step === 1" v-model:name="name" v-model:wallet="wallet" :state="state" :error="form.error" />
      <ProgramRulesStep v-else v-model:contact-id="contactId" :state="state" :error="form.error" :wallet="wallet" />
    </form>

    <template #footer>
      <Button variant="outline" class="max-md:h-11" @click="step === 1 ? (open = false) : (step = 1)">
        {{ step === 1 ? t('actions.cancel') : t('actions.back') }}
      </Button>
      <Button
        form="program-create-form"
        type="submit"
        class="max-md:h-11 max-md:flex-1"
        :disabled="save.isPending.value"
        :data-wizard-next="step === 1 || undefined"
        :data-wizard-save="step === 2 || undefined"
      >
        {{ step === 1 ? t('actions.next') : t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
