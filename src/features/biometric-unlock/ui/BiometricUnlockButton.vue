<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'
import { useRouter } from 'vue-router'
import { Fingerprint, Info } from 'lucide-vue-next'

import { parseError } from '@/shared/api/ipc'
import { useVaultStore } from '@/shared/store/vault'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'

import { type BiometricNotice, noticeFor } from '../lib/biometric-notice'
import { useBiometricRefresh, useBiometricState } from '../model/use-biometric'

/**
 * `Sign in with biometrics` under `Open` on the unlock card (Unlock 1 · 5-9). On
 * a system without a hardware-bound store nothing is drawn: the button has no
 * slot there, and the card is the same as before.
 */
const props = defineProps<{ path: string; disabled?: boolean }>()

const { t } = useI18n()
const router = useRouter()
const vault = useVaultStore()
const refresh = useBiometricRefresh()
const state = useBiometricState(() => props.path)

const notice = ref<BiometricNotice | null>(null)
watch(
  () => props.path,
  () => (notice.value = null),
)

const unlock = useMutation({
  mutationFn: () => vault.unlockByBiometric(props.path),
  onSuccess: () => router.replace({ name: 'dashboard' }),
  onError: (err) => {
    notice.value = noticeFor(parseError(err).code)
    refresh()
  },
  meta: { silent: 'the unlock card shows the reason under the button' },
})

const shown = computed(() => state.data.value === 'enabled' || state.data.value === 'password_required')
const expired = computed(() => state.data.value === 'password_required' || notice.value === 'password')
const info = computed(() => (notice.value === 'reset' ? 'reset' : expired.value ? 'password' : null))
</script>

<template>
  <div v-if="shown || notice === 'reset'" class="grid gap-3" data-biometric-unlock>
    <Button
      v-if="shown"
      type="button"
      variant="outline"
      size="lg"
      class="w-full gap-2"
      :disabled="props.disabled || expired || unlock.isPending.value"
      :loading="unlock.isPending.value"
      @click="unlock.mutate()"
    >
      <Fingerprint class="size-4" />
      {{ t('biometric.signIn') }}
    </Button>
    <Alert v-if="notice === 'failed'" variant="destructive">
      <AlertTitle>{{ t('biometric.failedTitle') }}</AlertTitle>
      <AlertDescription>{{ t('biometric.failedText') }}</AlertDescription>
    </Alert>
    <Alert v-else-if="info">
      <Info />
      <AlertTitle>{{ t(info === 'reset' ? 'biometric.resetTitle' : 'biometric.passwordTitle') }}</AlertTitle>
      <AlertDescription>{{ t(info === 'reset' ? 'biometric.resetText' : 'biometric.passwordText') }}</AlertDescription>
    </Alert>
  </div>
</template>
