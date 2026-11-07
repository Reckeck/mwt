<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { required, useFormValidation } from '@/shared/lib/form'
import { parseError } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'

import { connectBybit } from './connect-bybit'

/**
 * The two-field form that stores a Bybit key on this device. The backend asks Bybit for the key's
 * rights first: a key that can trade or withdraw is refused, and the reason goes under the key field
 * with both values kept, so the user can paste a better key without retyping the secret.
 */
const props = defineProps<{ storeId: string }>()
const emit = defineEmits<{ saved: []; refused: [] }>()

const { t } = useI18n()

const apiKey = ref('')
const apiSecret = ref('')
const busy = ref(false)
/** The backend's refusal of the key itself, shown under the key field until it is edited. */
const keyRefusal = ref<string | null>(null)
watch(apiKey, () => {
  keyRefusal.value = null
})

const form = useFormValidation(() => ({ apiKey: required(apiKey.value), apiSecret: required(apiSecret.value) }))

async function save() {
  busy.value = true
  try {
    const count = await connectBybit(props.storeId, apiKey.value, apiSecret.value, {
      funding: t('subType.funding'),
      unified: t('subType.unified'),
      earn: t('subType.earn'),
    })
    apiKey.value = ''
    apiSecret.value = ''
    form.reset()
    notify.success('wallet.bybit.keySaved', { description: t('wallet.bybit.accountsReady', { count }) })
    emit('saved')
  } catch (err) {
    const { code } = parseError(err)
    if (code === 'EXCHANGE_KEY_NOT_READ_ONLY' || code === 'EXCHANGE_KEY_REJECTED') {
      keyRefusal.value = t(`errors.${code}`)
    } else {
      notify.error(err, 'wallet.bybit.keySaveFailed')
      if (code === 'SECURITY_LEVEL_REQUIRED') emit('refused')
    }
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <!-- Key, secret and the button on one row: it is a two-field form. -->
  <form class="grid items-start gap-3 sm:grid-cols-[1fr_1fr_auto]" @submit.prevent="form.submit(save, $event)">
    <FormField
      for="bk"
      :label="t('wallet.bybit.apiKey')"
      required
      :error="form.error('apiKey') ?? keyRefusal ?? undefined"
      :hint="t('securityKeys.fieldHint')"
    >
      <Input
        id="bk"
        v-model="apiKey"
        autocomplete="off"
        spellcheck="false"
        :placeholder="t('securityKeys.keyPlaceholder')"
      />
    </FormField>
    <FormField for="bs" :label="t('wallet.bybit.apiSecret')" required :error="form.error('apiSecret')">
      <Input
        id="bs"
        v-model="apiSecret"
        type="password"
        autocomplete="new-password"
        spellcheck="false"
        :placeholder="t('securityKeys.secretPlaceholder')"
      />
    </FormField>
    <Button type="submit" class="sm:mt-[1.375rem]" :disabled="busy" :aria-busy="busy || undefined">
      {{ t('securityKeys.connect') }}
    </Button>
  </form>
</template>
