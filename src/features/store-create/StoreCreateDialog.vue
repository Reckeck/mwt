<script setup lang="ts">
import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { WizardSteps } from '@/shared/ui/wizard-steps'

import { useCreateStore } from './model/use-create-store'
import { provideStoreCreate } from './model/use-store-create-state'
import type { StoreCreateDialogProps } from './store-create.types'
import { ExchangeProviderFields } from './ui/exchange-provider-fields'
import { ExchangeSubFields } from './ui/exchange-sub-fields'
import { StoreBindingFields } from './ui/store-binding-fields'
import { StoreKindFields } from './ui/store-kind-fields'
import { StoreKindPicker } from './ui/store-kind-picker'
import { StoreMoneyFields } from './ui/store-money-fields'
import { StoreNameFields } from './ui/store-name-fields'

/**
 * A new wallet in three steps (Wallets-1 · 2–4): what it is, how it is called,
 * what is there now. A sub-account: its type, then its fields. State and checks
 * live in `model/use-store-create-state`, the sections in `ui/`.
 */
const props = withDefaults(defineProps<StoreCreateDialogProps>(), { parent: null, parentPath: () => [], kind: null })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ created: [store: Store] }>()

const { t } = useI18n()
const router = useRouter()
const state = provideStoreCreate(props, open)
const { step, steps, stepNumber, isLast, isSub, kind, name, nameTouched, title, canGoBack, goBack, goNext, form, exchangeSub, programOpen } = state
const createStore = useCreateStore(state, (store) => emit('created', store))

const stepTitles = computed(() => steps.value.map((entry) => t(`storeWizard.step.${isSub.value && entry === 'pick' ? 'subType' : entry}`)))

/** `Loyalty and bonuses`: the Bonuses wizard (name, card, point value) takes over from step 1, on its own page. */
watch(programOpen, (handed) => {
  if (!handed) return
  programOpen.value = false
  open.value = false
  void router.push({ name: 'transactions-bonuses', query: { new: 'program' } })
})

/** Each step checks its own fields: `Next` moves on, the last one creates. */
function submit(event?: Event) {
  if (step.value === 'pick') return goNext()
  form.submit(() => (isLast.value ? createStore.mutate() : goNext()), event)
}
</script>

<template>
  <Dialog overlay="store-create" v-model:open="open" :title="title" size="form">
    <div class="@container grid gap-4">
      <WizardSteps v-if="steps.length > 1" :step="stepNumber" :titles="stepTitles" />

      <StoreKindPicker v-if="step === 'pick'" />

      <form v-else id="store-create-form" class="grid gap-4" @submit.prevent="submit($event)">
        <ExchangeSubFields v-if="exchangeSub" />
        <template v-else-if="isSub">
          <StoreBindingFields />
          <FormField for="store-name" :label="t('stores.name')" required :error="form.error('name')">
            <Input id="store-name" v-model="name" :placeholder="t('stores.namePlaceholder')" @input="nameTouched = true" />
          </FormField>
          <StoreKindFields />
          <StoreMoneyFields />
        </template>
        <template v-else-if="step === 'details'">
          <ExchangeProviderFields v-if="kind === 'exchange'" />
          <StoreNameFields />
          <StoreKindFields />
        </template>
        <StoreMoneyFields v-else />
      </form>
    </div>

    <template #footer>
      <Button variant="outline" :class="['max-md:h-11', !canGoBack && 'max-md:hidden']" @click="canGoBack ? goBack() : (open = false)">
        {{ canGoBack ? t('actions.back') : t('actions.cancel') }}
      </Button>
      <Button
        :type="step === 'pick' ? 'button' : 'submit'"
        :form="step === 'pick' ? undefined : 'store-create-form'"
        class="max-md:h-11 max-md:flex-1"
        :loading="createStore.isPending.value"
        :data-wizard-next="!isLast || undefined"
        :data-wizard-save="isLast || undefined"
        @click="step === 'pick' && submit()"
      >
        {{ isLast && step !== 'pick' ? t('stores.createAction') : t('actions.next') }}
      </Button>
    </template>
  </Dialog>
</template>
