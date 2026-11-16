<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { Alert, AlertDescription } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { SettingsSection } from '@/shared/ui/settings-section'
import { useFormValidation } from '@/shared/lib/form'

import { validateEndpoint, validateKey } from '../lib/endpoint'
import { refusalText } from '../model/use-crypto-network'

/**
 * The addresses of one network (Settings · Crypto · Networks). The save button stays active: a wrong address
 * shows its message under the field and takes the focus (rule 13). Empty is the preset. The Blockscout key is
 * optional, never shown again after it is saved and kept only on this device.
 */
const props = defineProps<{ network: NetworkView; editable: boolean; pending: boolean; testing: boolean }>()
const emit = defineEmits<{
  save: [input: SetNetworkSettingsInput]
  test: [input: TestNetworkInput]
}>()

const { t } = useI18n()

const own = (endpoint: EndpointView) => (endpoint.custom ? endpoint.value : '')
const rpc = ref('')
const blockscout = ref('')
const explorer = ref('')
const key = ref('')
const refusal = ref('')

function load() {
  rpc.value = own(props.network.rpc)
  blockscout.value = own(props.network.blockscout)
  explorer.value = own(props.network.explorer)
  key.value = ''
  refusal.value = ''
}
watch(() => props.network, load, { immediate: true })

const form = useFormValidation(
  () => ({
    rpc: validateEndpoint('rpc', rpc.value),
    blockscout: validateEndpoint('blockscout', blockscout.value),
    explorer: validateEndpoint('explorer', explorer.value),
    key: validateKey(key.value),
  }),
  { resetOn: () => props.network.chain },
)

const scoutHint = computed(() =>
  props.network.blockscout.presets.length
    ? t('cryptoTrack.network.blockscoutHint', { preset: props.network.blockscout.presets[0] })
    : t('cryptoTrack.network.blockscoutNone'),
)
const keyHint = computed(() => (props.network.blockscout_key_set ? t('cryptoTrack.network.keyStored') : t('cryptoTrack.network.keyHint')))

function input(overrides: Partial<SetNetworkSettingsInput> = {}): SetNetworkSettingsInput {
  return {
    chain: props.network.chain,
    rpc: rpc.value.trim(),
    blockscout: blockscout.value.trim(),
    explorer: explorer.value.trim(),
    // Untouched field: the stored key stays. Typed: replaces it. Removed on its own button.
    blockscout_key: key.value.trim() ? key.value.trim() : undefined,
    ...overrides,
  }
}

function save() {
  refusal.value = ''
  emit('save', input())
}

function reset() {
  rpc.value = ''
  blockscout.value = ''
  explorer.value = ''
  form.reset()
  emit('save', input({ rpc: '', blockscout: '', explorer: '', blockscout_key: undefined }))
}

function removeKey() {
  key.value = ''
  emit('save', input({ blockscout_key: '' }))
}

function test(kind: EndpointKind) {
  const url = kind === 'rpc' ? rpc.value : blockscout.value
  if (validateEndpoint(kind, url) || validateKey(key.value)) {
    form.submit(() => undefined)
    return
  }
  emit('test', {
    chain: props.network.chain,
    kind,
    url: url.trim() || undefined,
    blockscout_key: key.value.trim() || undefined,
  })
}

defineExpose({ showRefusal: (err: unknown) => (refusal.value = refusalText(err)) })
</script>

<template>
  <SettingsSection
    :title="t('cryptoTrack.network.editTitle', { name: props.network.label })"
    :description="t('cryptoTrack.network.editHint')"
  >
    <Alert v-if="!props.editable" data-testid="network-level-required">
      <AlertDescription>{{ t('cryptoTrack.network.levelRequired') }}</AlertDescription>
    </Alert>
    <form class="flex flex-col gap-4" data-tour="crypto-network-form" novalidate @submit.prevent="form.submit(save)">
      <FormField
        for="crypto-network-rpc"
        :label="t('cryptoTrack.network.rpc')"
        :error="form.error('rpc')"
        :hint="t('cryptoTrack.network.rpcHint', { preset: props.network.rpc.presets[0] ?? '' })"
      >
        <Input
          id="crypto-network-rpc"
          v-model="rpc"
          :disabled="!props.editable"
          inputmode="url"
          autocomplete="off"
          spellcheck="false"
          :placeholder="props.network.rpc.presets[0]"
        />
      </FormField>
      <div v-if="props.network.rpc.presets.length > 1" class="-mt-2 flex flex-wrap items-center gap-1" data-tour="crypto-network-presets">
        <span class="text-sm text-muted-foreground">{{ t('cryptoTrack.network.presets') }}</span>
        <Button
          v-for="preset in props.network.rpc.presets"
          :key="preset"
          type="button"
          variant="ghost"
          size="sm"
          :disabled="!props.editable"
          @click="rpc = preset"
        >{{ preset.replace('https://', '') }}</Button>
      </div>
      <FormField
        for="crypto-network-blockscout"
        :label="t('cryptoTrack.network.blockscout')"
        :error="form.error('blockscout')"
        :hint="scoutHint"
      >
        <Input
          id="crypto-network-blockscout"
          v-model="blockscout"
          :disabled="!props.editable"
          inputmode="url"
          autocomplete="off"
          spellcheck="false"
          :placeholder="props.network.blockscout.presets[0]"
        />
      </FormField>
      <FormField
        for="crypto-network-explorer"
        :label="t('cryptoTrack.network.explorer')"
        :error="form.error('explorer')"
        :hint="t('cryptoTrack.network.explorerHint', { preset: props.network.explorer.presets[0] ?? '' })"
      >
        <Input
          id="crypto-network-explorer"
          v-model="explorer"
          :disabled="!props.editable"
          inputmode="url"
          autocomplete="off"
          spellcheck="false"
          :placeholder="props.network.explorer.presets[0]"
        />
      </FormField>
      <FormField
        for="crypto-network-key"
        :label="t('cryptoTrack.network.key')"
        :error="form.error('key')"
        :hint="keyHint"
      >
        <Input
          id="crypto-network-key"
          v-model="key"
          type="password"
          name="blockscout-key"
          :disabled="!props.editable"
          autocomplete="off"
          spellcheck="false"
          data-tour="crypto-network-key"
          :placeholder="props.network.blockscout_key_set ? t('cryptoTrack.network.keySetPlaceholder') : t('cryptoTrack.network.keyPlaceholder')"
        />
      </FormField>
      <p v-if="refusal" class="text-sm text-destructive" role="alert">{{ refusal }}</p>
      <div class="flex flex-wrap items-center gap-2">
        <Button type="submit" :disabled="props.pending">{{ t('actions.save') }}</Button>
        <Button type="button" variant="outline" :disabled="props.testing" @click="test('rpc')">{{ t('cryptoTrack.network.testRpc') }}</Button>
        <Button type="button" variant="outline" :disabled="props.testing" @click="test('blockscout')">{{ t('cryptoTrack.network.testScout') }}</Button>
        <Button type="button" variant="ghost" :disabled="props.pending || !props.editable" @click="reset">{{ t('cryptoTrack.network.reset') }}</Button>
        <ConfirmButton
          v-if="props.network.blockscout_key_set"
          :label="t('cryptoTrack.network.keyRemove')"
          :confirm-label="t('cryptoTrack.network.keyRemoveAsk')"
          :description="t('cryptoTrack.network.keyRemoveBody')"
          :action-label="t('cryptoTrack.network.keyRemove')"
          show-label
          @confirm="removeKey"
        />
      </div>
    </form>
  </SettingsSection>
</template>
