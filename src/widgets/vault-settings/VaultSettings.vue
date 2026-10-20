<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { KeyRound } from 'lucide-vue-next'

import { ChangePasswordDialog } from '@/features/vault-change-password'
import { VaultLinkSection } from '@/features/vault-external-link'
import { Button } from '@/shared/ui/button'
import { Separator } from '@/shared/ui/separator'
import { SettingsSection } from '@/shared/ui/settings-section'

/** The vault file: its password first, then what it is linked to. Device settings live in «Это устройство». */
const { t } = useI18n()
const passwordOpen = ref(false)
</script>

<template>
  <!-- Groups divided by a line, not boxed: a frame in a card in the window ate the width on a phone. -->
  <SettingsSection :title="t('settings.vaultTitle')" :description="t('settings.vaultScopeHint')">
    <section class="grid gap-2">
      <h4 class="text-sm font-medium">{{ t('settings.security') }}</h4>
      <p class="text-xs text-muted-foreground">{{ t('settings.securityHint') }}</p>
      <Button variant="outline" size="sm" class="w-fit" @click="passwordOpen = true">
        <KeyRound class="size-4" />
        {{ t('vault.password.open') }}
      </Button>
    </section>
    <Separator />
    <VaultLinkSection />
    <p class="text-xs text-muted-foreground">{{ t('settings.autoSaveHint') }}</p>
    <ChangePasswordDialog v-model:open="passwordOpen" />
  </SettingsSection>
</template>
