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
      <Button v-if="settings.app?.enrichment_opt_in" variant="outline" size="sm" @click="turnOff">
        {{ t('enrichment.turnOff') }}
      </Button>
      <Button v-else size="sm" @click="settings.enrichmentConsentOpen = true">{{ t('enrichment.turnOn') }}</Button>
    </div>
  </Card>
</template>
