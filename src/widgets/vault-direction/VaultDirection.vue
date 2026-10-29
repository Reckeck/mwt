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
 * `Direction`: whether the vault keeps crypto. Off hides crypto wallets,
 * exchanges and coins everywhere; their data and balances stay.
 */
const { t } = useI18n()
const settings = useSettingsStore()
const tree = useStoreTree()

const cryptoWallets = computed(
  () => (tree.data.value ?? []).filter((node) => !node.store.archived && isCryptoKind(node.store.kind)).length,
)

/** `If turned off: … Will be hidden: 2` when on, `Turned off: … Hidden: 2` when off — one line of the canvas: what the switch does and how many wallets it touches. */
const hint = computed(() => {
  const base = t(crypto.value ? 'settings.direction.cryptoHintOn' : 'settings.direction.cryptoHint')
  if (!cryptoWallets.value) return base
  return `${base} ${t(crypto.value ? 'settings.direction.willHide' : 'settings.direction.hiddenNow', { count: cryptoWallets.value })}`
})

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
    <SwitchRow
      id="vault-direction-crypto"
      v-model="crypto"
      plain
      :label="t('settings.direction.crypto')"
      :hint="hint"
      :disabled="toggle.isPending.value"
    />
  </SettingsSection>
</template>
