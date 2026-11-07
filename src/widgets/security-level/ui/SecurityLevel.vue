<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { CircleAlert, ShieldAlert } from 'lucide-vue-next'

import { useSecurityState } from '@/shared/api/security-state'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'
import { useVaultStore } from '@/shared/store/vault'
import { Skeleton } from '@/shared/ui/skeleton'

/**
 * The protection level of the open vault and what it protects (Security 1 · 1-6). There is no switch:
 * level 3 starts from connecting Bybit, and there is no button to lower the level, only the reason and
 * the way back (erasing the keys and crypto data, text only for now).
 */
const { t } = useI18n()
const security = useSecurityState()
const vault = useVaultStore()
</script>

<template>
  <SettingsSection v-if="!vault.sandbox" :title="t('securityLevel.title')" data-section="security-level" data-tour="security-level">
    <!-- Same height as the card it becomes, so nothing below moves when the answer arrives. -->
    <div v-if="security.isPending.value" class="grid gap-2" :aria-busy="true">
      <Skeleton class="h-5 w-40" />
      <Skeleton class="h-4 w-full" />
      <Skeleton class="h-4 w-3/4" />
    </div>
    <Alert v-else-if="security.error.value || !security.data.value" variant="destructive">
      <CircleAlert />
      <AlertTitle>{{ t('securityLevel.loadFailed') }}</AlertTitle>
      <AlertDescription class="flex flex-col items-start gap-2">
        <span>{{ t('securityLevel.loadFailedHint') }}</span>
        <Button variant="outline" size="sm" @click="security.refetch()">{{ t('actions.retry') }}</Button>
      </AlertDescription>
    </Alert>
    <template v-else>
      <div class="grid gap-1" :data-level="security.data.value.level">
        <p class="font-medium">{{ t(`securityLevel.name.${security.data.value.level}`) }}</p>
        <p class="text-sm text-muted-foreground">{{ t(`securityLevel.text.${security.data.value.level}`) }}</p>
      </div>
      <p v-if="security.data.value.level !== 'crypto'" class="text-sm text-muted-foreground">{{ t('securityLevel.howTo') }}</p>
      <Alert v-else-if="security.data.value.has_device_keys">
        <ShieldAlert />
        <AlertTitle>{{ t('securityLevel.noDowngrade.title') }}</AlertTitle>
        <AlertDescription>{{ t('securityLevel.noDowngrade.text') }}</AlertDescription>
      </Alert>
      <p v-else class="text-sm text-muted-foreground">{{ t('securityLevel.noKeys') }}</p>
    </template>
  </SettingsSection>
</template>
