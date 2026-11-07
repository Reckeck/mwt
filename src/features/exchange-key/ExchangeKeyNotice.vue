<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ShieldAlert } from 'lucide-vue-next'

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'

import { openRaiseWizard } from './open-raise-wizard'

/**
 * Why the key cannot be used here: the vault is below level 3 (with or without an old connection),
 * or the key was stored on another device and does not travel.
 */
const props = defineProps<{
  state: ExchangeKeyState
  /** The wallet was connected before (an old key below level 3). */
  connected: boolean
  /** The wallet has Bybit sub-accounts but this device holds no key: it was connected elsewhere. */
  elsewhere: boolean
}>()

const { t } = useI18n()
const router = useRouter()

const kind = computed(() => {
  if (props.state === 'level_required') return props.connected ? 'stopped' : 'level'
  if (props.state === 'missing' && props.elsewhere) return 'elsewhere'
  return null
})
</script>

<template>
  <div v-if="kind" class="grid gap-3">
    <Alert>
      <ShieldAlert />
      <AlertTitle>{{ t(`securityKeys.notice.${kind}.title`) }}</AlertTitle>
      <AlertDescription>{{ t(`securityKeys.notice.${kind}.text`) }}</AlertDescription>
    </Alert>
    <Button
      v-if="kind !== 'elsewhere'"
      class="w-fit"
      data-tour="security-raise-start"
      @click="openRaiseWizard(router)"
    >
      {{ t('securityKeys.raise') }}
    </Button>
  </div>
</template>
