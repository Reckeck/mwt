<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { call, parseError } from '@/shared/api/ipc'
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

/** Wayland gives apps no global keys: there the compositor binds a key to a command. */
const support = useQuery({ queryKey: ['shortcut-support'], queryFn: () => call<ShortcutSupport>('get_shortcut_support') })
const binding = computed(() => `bind = SUPER SHIFT, A, exec, ${support.data.value?.launch_command ?? 'trade-app --quick-add'}`)
async function copyBinding() {
  try {
    await navigator.clipboard.writeText(binding.value)
    toast.success(t('quickAdd.copied'))
  } catch {
    /* the line stays visible to copy by hand */
  }
}

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
    <div v-if="support.data.value && !support.data.value.global_keys" class="grid gap-2">
      <p class="text-sm">{{ t('quickAdd.waylandHint') }}</p>
      <code class="block overflow-x-auto rounded-md bg-muted px-3 py-2 font-mono text-xs">{{ binding }}</code>
      <div class="flex flex-wrap items-center gap-2">
        <Button size="sm" variant="outline" @click="copyBinding">{{ t('quickAdd.copy') }}</Button>
        <span class="text-xs text-muted-foreground">{{ t('quickAdd.waylandWhere') }}</span>
      </div>
    </div>
    <div v-else class="grid gap-3">
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
