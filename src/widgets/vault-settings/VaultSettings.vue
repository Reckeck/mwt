<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { KeyRound } from 'lucide-vue-next'

import { ChangePasswordDialog } from '@/features/vault-change-password'
import { VaultLinkSection } from '@/features/vault-external-link'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'

/** The vault file: its password first, then what it is linked to. Device settings live in «Это устройство». */
const { t } = useI18n()
const passwordOpen = ref(false)
</script>

<template>
  <Card>
    <template #header>
      <h3 class="heading-card">{{ t('settings.vaultTitle') }}</h3>
    </template>

    <div class="grid gap-4">
      <p class="text-xs text-muted-foreground">{{ t('settings.vaultScopeHint') }}</p>

      <section class="grid gap-2 rounded-md border border-border px-3 py-3">
        <h4 class="text-sm font-medium">{{ t('settings.security') }}</h4>
        <p class="text-xs text-muted-foreground">{{ t('settings.securityHint') }}</p>
        <Button variant="outline" size="sm" class="w-fit" @click="passwordOpen = true">
          <KeyRound class="size-4" />
          {{ t('vault.password.open') }}
        </Button>
      </section>

      <p class="text-xs text-muted-foreground">{{ t('settings.autoSaveHint') }}</p>

      <VaultLinkSection />
    </div>
    <ChangePasswordDialog v-model:open="passwordOpen" />
  </Card>
</template>
