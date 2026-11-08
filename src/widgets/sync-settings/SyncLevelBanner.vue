<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ShieldAlert } from 'lucide-vue-next'

import { RaiseWizard } from '@/features/security-raise'
import { useSyncLevelRefusal } from '@/features/vault-sync'
import { useSecurityState } from '@/shared/api/security-state'
import { openRaiseWizard } from '@/shared/lib/raise-wizard-state'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'

/**
 * This device refused crypto data from a paired one: it is below level 3. Stays until the vault reaches
 * level 3 (the refusal is remembered for the session by `useSyncLevelRefusal`); the button opens the wizard.
 */
const { t } = useI18n()
const { refusal } = useSyncLevelRefusal()
const security = useSecurityState()

const shown = computed(() => refusal.here && security.data.value !== undefined && security.data.value.level !== 'crypto')
</script>

<template>
  <Alert v-if="shown" class="border-warning/40 bg-warning/10 text-foreground" data-tour="sync-level-required" data-sync-level-required>
    <ShieldAlert class="text-warning" />
    <AlertTitle>{{ t('sync.levelRequired.title') }}</AlertTitle>
    <AlertDescription class="grid gap-2">
      <span>{{ t('sync.levelRequired.text') }}</span>
      <Button size="sm" class="justify-self-start" data-tour="security-raise-start" @click="openRaiseWizard()">
        {{ t('sync.levelRequired.raise') }}
      </Button>
    </AlertDescription>
  </Alert>
  <!-- Outside the banner: it stays open on the last step after level 3 hides the banner. -->
  <RaiseWizard />
</template>
