<script setup lang="ts">
import { useMutation } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { usePlatform } from '@/shared/lib/platform'
import { useVaultStore } from '@/shared/store/vault'
import { Button } from '@/shared/ui/button'

/** Where the vault file lies, with a way to show its folder (desktop only). */
const { t } = useI18n()
const vault = useVaultStore()
const { isDesktop } = usePlatform()

const reveal = useMutation({
  mutationFn: () => call<void>('reveal_vault_folder'),
  meta: { success: '', error: 'vaultFile.revealFailed' },
})
</script>

<template>
  <div class="grid gap-1.5">
    <span class="text-sm font-medium">{{ t('vaultFile.whereTitle') }}</span>
    <div class="flex min-w-0 items-center justify-between gap-3 rounded-md border border-border px-3 py-1.5">
      <span class="min-w-0 break-all text-sm" data-vault-path>{{ vault.status?.path }}</span>
      <Button v-if="isDesktop" variant="secondary" size="sm" class="shrink-0" :loading="reveal.isPending.value" @click="reveal.mutate()">
        {{ t('vaultFile.reveal') }}
      </Button>
    </div>
  </div>
</template>
