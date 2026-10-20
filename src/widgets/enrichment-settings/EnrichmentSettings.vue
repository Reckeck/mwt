<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { useSettingsStore } from '@/shared/store/settings'
import { notify } from '@/shared/lib/notify'

const { t } = useI18n()
const settings = useSettingsStore()

/** Turning it on goes through the consent dialog, so the user sees what leaves. */
async function turnOff() {
  try {
    await settings.updateApp({ enrichment_opt_in: false })
    notify.saved()
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
  }
}
</script>

<template>
  <Card>
    <template #header>
      <h3 class="heading-card">{{ t('enrichment.settingsTitle') }}</h3>
      <p class="text-sm text-muted-foreground">{{ t('enrichment.settingsHint') }}</p>
    </template>
    <div class="flex items-center justify-between gap-3">
      <span class="text-sm">
        {{ settings.app?.enrichment_opt_in ? t('enrichment.stateOn') : t('enrichment.stateOff') }}
      </span>
      <!-- One button for both states: turning it off keeps the focus where it was (rule 17). -->
      <Button
        :variant="settings.app?.enrichment_opt_in ? 'outline' : 'default'"
        size="sm"
        @click="settings.app?.enrichment_opt_in ? turnOff() : (settings.enrichmentConsentOpen = true)"
      >
        {{ settings.app?.enrichment_opt_in ? t('enrichment.turnOff') : t('enrichment.turnOn') }}
      </Button>
    </div>
  </Card>
</template>
