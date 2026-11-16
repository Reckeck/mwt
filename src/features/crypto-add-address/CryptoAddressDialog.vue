<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { RaiseWizard } from '@/features/security-raise/@x/crypto-add-address'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Alert, AlertDescription } from '@/shared/ui/alert'

import { useCryptoAddAddress } from './model/use-crypto-add-address'
import { CryptoAddressFields } from './ui/crypto-address-fields'
import { CryptoConsentStep } from './ui/crypto-consent-step'
import { CryptoStartFields } from './ui/crypto-start-fields'

/**
 * Follow a public address, read-only (mockups 08 to 11): the wizard's last two steps for a wallet
 * of kind `Crypto` (the first, "what is it", is the kind picker of `store-create`), and the same
 * dialog from the empty list's button. It asks for the one-time consent first, until it is given.
 * There is no field for a seed phrase or a private key (see `useCryptoAddAddress`).
 */
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ created: [storeId: string] }>()
const { t } = useI18n()
const state = useCryptoAddAddress(open, (storeId) => emit('created', storeId))
const { step, form, connect, consent } = state

const TITLES = { consent: 'cryptoTrack.connect.consentTitle', start: 'cryptoTrack.connect.startTitle', address: 'cryptoTrack.connect.title' }
const title = computed(() => t(TITLES[step.value]))
const description = computed(() =>
  step.value === 'consent'
    ? t('cryptoTrack.connect.consentLead')
    : t(step.value === 'start' ? 'cryptoTrack.connect.stepStart' : 'cryptoTrack.connect.stepAddress'),
)
const pending = computed(() => connect.isPending.value || consent.isPending.value)
const nextLabel = computed(() =>
  t(step.value === 'consent' ? 'cryptoTrack.connect.agree' : step.value === 'start' ? 'cryptoTrack.connect.submit' : 'actions.next'),
)
</script>

<template>
  <Dialog v-model:open="open" overlay="crypto-add-address" size="form" :title="title" :description="description">
    <form id="crypto-add-address-form" class="@container flex flex-col gap-4" data-tour="crypto-add-address-form" @submit.prevent="state.next($event)">
      <CryptoConsentStep v-if="step === 'consent'" />
      <CryptoAddressFields
        v-else-if="step === 'address'"
        v-model:chain="state.chain.value"
        v-model:name="state.name.value"
        :networks="state.networks"
        :address="state.address.value"
        :address-key="state.addressKey.value"
        :refused="state.refused.value"
        :address-error="state.addressError.value"
        :name-error="state.serverError.value?.field === 'name' ? state.serverError.value.message : undefined"
        @address="state.setAddress"
      />
      <CryptoStartFields
        v-else
        v-model:whole="state.wholeHistory.value"
        v-model:date="state.startDate.value"
        :error="form.error('start')"
      />
      <Alert v-if="state.serverError.value?.field === 'other'" variant="destructive">
        <AlertDescription>{{ state.serverError.value.message }}</AlertDescription>
      </Alert>
    </form>

    <template #footer>
      <Button variant="outline" type="button" class="max-md:h-11" @click="step === 'start' ? state.back() : (open = false)">
        {{ step === 'start' ? t('actions.back') : t('actions.cancel') }}
      </Button>
      <Button type="submit" form="crypto-add-address-form" class="max-md:h-11 max-md:flex-1" :loading="pending" :disabled="pending">
        {{ nextLabel }}
      </Button>
    </template>
  </Dialog>
  <RaiseWizard v-if="state.raising.value" @raised="state.onRaised()" />
</template>
