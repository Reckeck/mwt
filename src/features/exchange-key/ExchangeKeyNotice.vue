<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ShieldAlert } from 'lucide-vue-next'

import { openRaiseWizard, useRaiseWizardState } from '@/shared/lib/raise-wizard-state'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'

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
/** A move that was started and stopped: the page says so and the button continues it. */
const wizard = useRaiseWizardState()

const kind = computed(() => {
  if (props.state === 'level_required') {
    if (wizard.interrupted) return 'interrupted'
    return props.connected ? 'stopped' : 'level'
  }
  if (props.state === 'missing' && props.elsewhere) return 'elsewhere'
  return null
})
</script>

<template>
  <div v-if="kind" class="grid gap-3">
    <Alert>
      <ShieldAlert />
      <AlertTitle>{{ kind === 'interrupted' ? t('securityRaise.interrupted.title') : t(`securityKeys.notice.${kind}.title`) }}</AlertTitle>
      <AlertDescription>
        {{ kind === 'interrupted' ? t('securityRaise.interrupted.text') : t(`securityKeys.notice.${kind}.text`) }}
      </AlertDescription>
    </Alert>
    <Button
      v-if="kind !== 'elsewhere'"
      class="w-fit"
      data-tour="security-raise-start"
      @click="openRaiseWizard()"
    >
      {{ kind === 'interrupted' ? t('securityRaise.resume') : t('securityKeys.raise') }}
    </Button>
  </div>
</template>
