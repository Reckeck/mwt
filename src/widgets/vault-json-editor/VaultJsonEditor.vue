<script setup lang="ts">
/// Read/write raw JSON view of the unlocked vault document.
///
/// The editor is CodeMirror (`shared/lib/editor/`), which owns highlighting,
/// undo history, bracket handling, folding and the completion popup; the draft,
/// its validation and the writes live in `model/use-vault-json-editor`.
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { EDITOR_HEIGHT } from '@/shared/lib/editor'

import { useVaultJsonEditor } from './model/use-vault-json-editor'
import { VaultJsonToolbar } from './ui/vault-json-toolbar'

const { t } = useI18n()
const mountEl = ref<HTMLDivElement | null>(null)
const { status, syntaxError, scratchPath, loading, resetToApplied, openInEditor, reloadFromEditor } = useVaultJsonEditor(mountEl)
</script>

<template>
  <div class="grid gap-3">
    <VaultJsonToolbar
      :status="status"
      :loading="loading"
      :has-scratch="Boolean(scratchPath)"
      @open-in-editor="openInEditor"
      @reload-from-editor="reloadFromEditor"
      @reset="resetToApplied"
    />

    <!-- Syntax errors stay inline: they track the caret and are read next to the draft. -->
    <p v-if="syntaxError" class="text-xs text-destructive">{{ t('settings.dev.jsonInvalid') }}: {{ syntaxError }}</p>
    <p v-if="scratchPath" class="break-all text-xs text-muted-foreground">{{ t('settings.dev.scratchFile') }}: {{ scratchPath }}</p>

    <div ref="mountEl" class="overflow-hidden rounded-md border border-border bg-muted/50 font-mono" :style="{ height: EDITOR_HEIGHT }" />

    <p class="text-xs text-muted-foreground">{{ t('settings.dev.saveHint') }}</p>
    <p class="text-xs text-muted-foreground">{{ t('settings.dev.autocompleteHint') }}</p>
  </div>
</template>
