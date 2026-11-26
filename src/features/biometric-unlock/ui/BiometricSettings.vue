<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { usePlatform } from '@/shared/lib/platform'
import { SettingsSection } from '@/shared/ui/settings-section'
import { SwitchRow } from '@/shared/ui/switch-row'

import { useBiometricDisable, useSecurityState } from '../model/use-biometric'
import BiometricEnableDialog from './BiometricEnableDialog.vue'

/**
 * `Storage` > biometric sign-in (Security 1 · 9-13). Level 3 only. Without a
 * hardware-bound store the card stays in place and explains, with no switch.
 */
const { t } = useI18n()
const { os } = usePlatform()
const security = useSecurityState()
const disable = useBiometricDisable()
const enableOpen = ref(false)

const state = computed(() => security.data.value?.biometric ?? 'unavailable')
const shown = computed(() => security.data.value?.level === 'crypto')
const on = computed(() => state.value === 'enabled' || state.value === 'password_required')
const status = computed(() => {
  if (state.value === 'password_required') return t('biometric.expired')
  return t(state.value === 'enabled' ? 'biometric.on' : 'biometric.off')
})

/** The switch never flips by itself: turning on waits for the password dialog, off for the command. */
function setOn(value: boolean) {
  if (value) enableOpen.value = true
  else disable.mutate()
}
</script>

<template>
  <SettingsSection v-if="shown" :title="t('biometric.title')">
    <p v-if="state === 'unavailable'" class="text-sm text-muted-foreground" data-biometric-unavailable>
      {{ t('biometric.unavailable') }}
    </p>
    <template v-else>
      <SwitchRow
        id="biometric-toggle"
        data-tour="biometric-toggle"
        plain
        :model-value="on"
        :label="t(os === 'windows' ? 'biometric.toggleWindows' : 'biometric.toggle')"
        :hint="t('biometric.toggleHint')"
        :disabled="disable.isPending.value"
        @update:model-value="setOn"
      />
      <p class="text-sm text-muted-foreground" data-biometric-status>{{ status }}</p>
    </template>
    <BiometricEnableDialog v-model:open="enableOpen" />
  </SettingsSection>
</template>
