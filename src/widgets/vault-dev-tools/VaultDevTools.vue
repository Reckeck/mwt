<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Braces } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'
import { Dialog } from '@/shared/ui/dialog'
import { call, parseError, type AppError } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'
import { useVaultStore } from '@/shared/store/vault'

/**
 * The viewer is a slot, not an import: the JSON editor is a widget of its own,
 * and widgets do not import each other — the page plugs it in.
 */
defineSlots<{ viewer(): unknown }>()

const { t } = useI18n()
// The sandbox's document is demo data in a temporary file: nothing to view, dump or edit by hand.
const vault = useVaultStore()

const jsonDialogOpen = ref(false)

async function devDumpVault() {
  try {
    // Native save dialog + write happen in Rust: the webview cannot download
    // files on its own, and `dev_export_vault` returns null when cancelled.
    const written = await call<string | null>('dev_export_vault')
    if (written) notify.success('settings.dev.savedTo', { description: written })
  } catch (err) {
    notify.error(null, 'settings.dev.dumpFailed', { description: errorText(parseError(err)) })
  }
}

/** The viewer loads the document itself on mount. */
function viewVaultJson() {
  jsonDialogOpen.value = true
}

/** Show a known error code as its localized text; fall back to the raw message. */
function errorText(err: AppError): string {
  const known = t(`errors.${err.code}`)
  return known === `errors.${err.code}` ? err.message : known
}
</script>

<template>
  <div class="contents">
    <SettingsSection :title="t('settings.dev.openVault')">
      <div class="flex flex-wrap items-center gap-2">
        <Badge variant="secondary" class="text-xs">DEV</Badge>
        <Button variant="outline" :disabled="vault.sandbox" @click="viewVaultJson">
          <Braces class="size-4" />
          {{ t('settings.dev.viewVault') }}
        </Button>
        <Button variant="outline" :disabled="vault.sandbox" @click="devDumpVault">
          {{ t('settings.dev.dumpVault') }}
        </Button>
      </div>
      <p class="text-xs text-muted-foreground">{{ vault.sandbox ? t('sandbox.settingOff') : t('settings.dev.dumpHint') }}</p>
    </SettingsSection>

    <Dialog overlay="dev-tools"
      v-model:open="jsonDialogOpen"
      :title="t('settings.dev.vaultJsonTitle')"
      :description="t('settings.dev.vaultJsonHint')"
      size="xl"
    >
      <!-- Mounted per open so every visit starts from the live document. -->
      <slot v-if="jsonDialogOpen" name="viewer" />
      <template #footer>
        <Button @click="jsonDialogOpen = false">{{ t('actions.close') }}</Button>
      </template>
    </Dialog>
  </div>
</template>
