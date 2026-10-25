<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { SettingsSection } from '@/shared/ui/settings-section'
import { SwitchRow } from '@/shared/ui/switch-row'
import { useSettingsStore } from '@/shared/store/settings'
import { notify } from '@/shared/lib/notify'

const { t } = useI18n()
const settings = useSettingsStore()
const open = ref(false)

/** Turning it on goes through the consent dialog, so the user sees what leaves. */
async function change(on: boolean) {
  if (on) {
    open.value = false
    settings.enrichmentConsentOpen = true
    return
  }
  try {
    await settings.updateApp({ enrichment_opt_in: false })
    notify.saved()
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
  }
}
</script>

<template>
  <SettingsSection :title="t('enrichment.settingsTitle')" :description="t('enrichment.settingsHint')">
    <div class="flex flex-wrap items-center gap-2">
      <Button variant="outline" @click="open = true">{{ t('enrichment.configure') }}</Button>
    </div>
    <Dialog overlay="enrichment-settings" v-model:open="open" :title="t('enrichment.settingsTitle')" :description="t('enrichment.stateHint')">
      <SwitchRow
        id="enrichment-opt-in"
        :label="t('enrichment.settingsTitle')"
        :model-value="settings.app?.enrichment_opt_in ?? false"
        @update:model-value="change"
      />
      <template #footer>
        <Button @click="open = false">{{ t('actions.close') }}</Button>
      </template>
    </Dialog>
  </SettingsSection>
</template>
