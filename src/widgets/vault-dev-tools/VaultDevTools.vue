<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Download, Eye } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Dialog } from '@/shared/ui/dialog'
import { call, parseError, type AppError } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'

/**
 * The viewer is a slot, not an import: the JSON editor is a widget of its own,
 * and widgets do not import each other — the page plugs it in.
 */
defineSlots<{ viewer(): unknown }>()

const { t } = useI18n()

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
    <Card>
      <template #header>
        <div class="flex items-center justify-between">
          <p class="font-medium">{{ t('settings.dev.openVault') }}</p>
          <Badge variant="secondary" class="text-xs">DEV</Badge>
        </div>
      </template>
      <div class="grid gap-2">
        <div class="flex flex-wrap items-center gap-2">
          <Button variant="outline" @click="viewVaultJson">
            <Eye class="size-4" />
            {{ t('settings.dev.viewVault') }}
          </Button>
          <Button variant="outline" @click="devDumpVault">
            <Download class="size-4" />
            {{ t('settings.dev.dumpVault') }}
          </Button>
          <p class="text-xs text-muted-foreground">{{ t('settings.dev.dumpHint') }}</p>
        </div>
      </div>
    </Card>

    <Dialog
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
