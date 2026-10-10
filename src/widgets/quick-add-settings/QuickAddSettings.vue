<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const settings = useSettingsStore()

const draft = ref(settings.app?.quick_add_shortcut ?? '')
watch(
  () => settings.app?.quick_add_shortcut,
  (value) => {
    draft.value = value ?? ''
  },
)
const failed = ref('')

const MODIFIER_CODES = new Set(['ControlLeft', 'ControlRight', 'ShiftLeft', 'ShiftRight', 'AltLeft', 'AltRight', 'MetaLeft', 'MetaRight'])

/** Pressing a combination in the field writes it in the form the backend parses. */
function record(event: KeyboardEvent) {
  if (event.key === 'Tab') return
  event.preventDefault()
  if (event.key === 'Backspace' || event.key === 'Delete') {
    draft.value = ''
    return
  }
  if (MODIFIER_CODES.has(event.code)) return
  const parts: string[] = []
  if (event.ctrlKey) parts.push('CmdOrCtrl')
  if (event.altKey) parts.push('Alt')
  if (event.shiftKey) parts.push('Shift')
  if (event.metaKey) parts.push('Super')
  parts.push(event.code.replace(/^Key/, '').replace(/^Digit/, ''))
  draft.value = parts.join('+')
}

async function save(shortcut: string | null) {
  failed.value = ''
  try {
    await settings.setQuickAddShortcut(shortcut)
    toast.success(t(shortcut ? 'quickAdd.shortcutSaved' : 'quickAdd.shortcutOff'))
  } catch (err) {
    const error = parseError(err)
    failed.value = error.code === 'SHORTCUT_FAILED' ? t('quickAdd.shortcutTaken') : codeErrorMessage(error, t)
  }
}
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('quickAdd.settingsTitle') }}</p>
      <p class="text-xs text-muted-foreground">{{ t('quickAdd.settingsHint') }}</p>
    </template>
    <div class="grid gap-3">
      <div class="grid gap-1.5">
        <Label for="quick-add-shortcut">{{ t('quickAdd.shortcut') }}</Label>
        <div class="flex flex-wrap gap-2">
          <Input
            id="quick-add-shortcut"
            v-model="draft"
            class="w-64 font-mono"
            :placeholder="t('quickAdd.shortcutPlaceholder')"
            @keydown="record"
          />
          <Button size="sm" class="h-9" :disabled="!draft || draft === settings.app?.quick_add_shortcut" @click="save(draft)">
            {{ t('actions.save') }}
          </Button>
          <Button v-if="settings.app?.quick_add_shortcut" variant="outline" size="sm" class="h-9" @click="save(null)">
            {{ t('quickAdd.turnOff') }}
          </Button>
        </div>
        <p class="text-xs text-muted-foreground">
          {{ settings.app?.quick_add_shortcut ? t('quickAdd.stateOn', { keys: settings.app.quick_add_shortcut }) : t('quickAdd.stateOff') }}
        </p>
        <p v-if="failed" class="text-xs text-destructive">{{ failed }}</p>
      </div>
    </div>
  </Card>
</template>
