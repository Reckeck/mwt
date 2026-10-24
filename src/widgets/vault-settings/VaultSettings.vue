<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { KeyRound } from 'lucide-vue-next'

import { VaultBackupsSection } from '@/features/vault-backups'
import { ChangePasswordDialog } from '@/features/vault-change-password'
import { VaultLinkSection } from '@/features/vault-external-link'
import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'

import VaultPlace from './VaultPlace.vue'

/** The vault file as four cards (Settings-1 · 5): where it lies, its password, the outside copy, backups. */
const { t } = useI18n()
const passwordOpen = ref(false)
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <SettingsSection :title="t('vaultFile.title')"><VaultPlace /></SettingsSection>
    <SettingsSection :title="t('settings.security')" :description="t('settings.securityHint')">
      <Button variant="outline" size="sm" class="w-fit gap-2" @click="passwordOpen = true">
        <KeyRound class="size-4" />
        {{ t('vault.password.open') }}
      </Button>
    </SettingsSection>
    <SettingsSection :title="t('vaultLink.title')" :description="t('vaultLink.hint')"><VaultLinkSection /></SettingsSection>
    <SettingsSection :title="t('vaultBackups.title')"><VaultBackupsSection /></SettingsSection>
    <ChangePasswordDialog v-model:open="passwordOpen" />
  </div>
</template>
