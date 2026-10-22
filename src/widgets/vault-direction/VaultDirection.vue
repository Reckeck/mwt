<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'

import { useStoreTree } from '@/shared/api/queries'
import { isCryptoKind } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'
import { SettingsSection } from '@/shared/ui/settings-section'
import { SwitchRow } from '@/shared/ui/switch-row'

/**
 * «Направление»: whether the vault keeps crypto. Off hides crypto wallets,
 * exchanges and coins everywhere; their data and balances stay.
 */
const { t } = useI18n()
const settings = useSettingsStore()
const tree = useStoreTree()

const cryptoWallets = computed(
  () => (tree.data.value ?? []).filter((node) => !node.store.archived && isCryptoKind(node.store.kind)).length,
)

const toggle = useMutation({
  mutationFn: (crypto: boolean) => settings.updateVault({ crypto }),
  meta: {
    success: () => t(settings.cryptoEnabled ? 'settings.direction.shown' : 'settings.direction.hidden'),
    error: 'settings.saveFailed',
  },
})

const crypto = computed({
  get: () => settings.cryptoEnabled,
  set: (value: boolean) => toggle.mutate(value),
})
</script>

<template>
  <SettingsSection :title="t('settings.direction.title')" :description="t('settings.direction.description')">
    <div class="grid gap-2">
      <SwitchRow
        id="vault-direction-crypto"
        v-model="crypto"
        :label="t('settings.direction.crypto')"
        :hint="t('settings.direction.cryptoHint')"
        :disabled="toggle.isPending.value"
      />
      <!-- Always rendered: the line reserves its place, so the switch does not move (rule 17). -->
      <p class="min-h-5 text-xs text-muted-foreground">
        {{
          cryptoWallets
            ? t(crypto ? 'settings.direction.willHide' : 'settings.direction.hiddenNow', { count: cryptoWallets })
            : ''
        }}
      </p>
    </div>
  </SettingsSection>
</template>
