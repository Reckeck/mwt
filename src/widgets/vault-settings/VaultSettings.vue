<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { KeyRound, ShieldOff } from 'lucide-vue-next'

import { VaultBackupsSection } from '@/features/vault-backups'
import { VaultLinkSection, useVaultLink } from '@/features/vault-external-link'
import { ChangePasswordDialog, RemovePasswordDialog, SetPasswordDialog } from '@/features/vault-password'
import { useVaultStore } from '@/shared/store/vault'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'

import VaultPlace from './VaultPlace.vue'

/** The vault file as four cards (Settings-1 · 5, Full-New-Encrypt 4–5): where it lies, its password, the outside copy, backups. */
const { t } = useI18n()
const vault = useVaultStore()
const { status: link } = useVaultLink()

const changeOpen = ref(false)
const removeOpen = ref(false)
const setOpen = ref(false)

/** Name of the external copy, `null` without one: the questions about the password tell it what changes. */
const externalCopy = computed(() => (link.data.value?.linked ? (link.data.value.display_name ?? '') : null))
</script>

<template>
  <div v-if="vault.sandbox" class="grid grid-cols-1 gap-4" data-vault-sandbox>
    <SettingsSection :title="t('sandbox.name')" :description="t('sandbox.openHintShort')">
      <p class="text-sm text-muted-foreground">{{ t('sandbox.vaultOff') }}</p>
    </SettingsSection>
  </div>
  <div v-else class="grid grid-cols-1 gap-4">
    <SettingsSection :title="t('vaultFile.title')"><VaultPlace /></SettingsSection>
    <SettingsSection
      :title="t('settings.security')"
      :description="t(vault.encrypted ? 'settings.securityHint' : 'vaultPassword.plainHint')"
      data-section="vault-security"
      data-tour="vault-security"
    >
      <template v-if="!vault.encrypted" #actions>
        <Badge variant="warning" class="gap-1 font-normal" data-vault-plain>
          <ShieldOff class="size-3" />
          {{ t('vaultPassword.plainBadge') }}
        </Badge>
      </template>
      <div v-if="vault.encrypted" class="flex flex-wrap gap-2">
        <Button variant="outline" size="sm" class="gap-2" @click="changeOpen = true">
          <KeyRound class="size-4" />
          {{ t('vault.password.open') }}
        </Button>
        <Button variant="outline" size="sm" @click="removeOpen = true">{{ t('vaultPassword.removeOpen') }}</Button>
      </div>
      <Button v-else size="sm" class="w-fit gap-2" @click="setOpen = true">
        <KeyRound class="size-4" />
        {{ t('vaultPassword.setOpen') }}
      </Button>
    </SettingsSection>
    <SettingsSection :title="t('vaultLink.title')" :description="t('vaultLink.hint')"><VaultLinkSection /></SettingsSection>
    <SettingsSection :title="t('vaultBackups.title')"><VaultBackupsSection /></SettingsSection>
    <ChangePasswordDialog v-model:open="changeOpen" />
    <RemovePasswordDialog v-model:open="removeOpen" :external-copy="externalCopy" />
    <SetPasswordDialog v-model:open="setOpen" :external-copy="externalCopy !== null" />
  </div>
</template>
