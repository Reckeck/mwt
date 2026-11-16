<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatBytes } from '@/entities/attachment'
import { formatDateTime } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { SettingsSection } from '@/shared/ui/settings-section'

import { useCryptoWipe } from '../model/use-crypto-wipe'
import CryptoWipeDialog from './CryptoWipeDialog.vue'

/**
 * Settings · Crypto · Wiping: the two steps of removing the module from the vault — a backup of the
 * vault first, then the wipe itself behind a confirmation that spells out what goes.
 *
 * The wipe takes a backup of its own and reads it back before it removes anything; the copy offered
 * here is the one the user can keep somewhere else beforehand.
 */
const { t } = useI18n()
const { preview, backups, makeBackup, wipe } = useCryptoWipe()
const askOpen = ref(false)

const latest = computed(() => backups.data.value?.[0] ?? null)
const latestLine = computed(() => {
  const backup = latest.value
  if (!backup) return ''
  const size = formatBytes(backup.size_bytes, { b: t('attachments.bytes'), kb: t('attachments.kb'), mb: t('attachments.mb') })
  return `${backup.name} · ${size}`
})
</script>

<template>
  <SectionState
    :loading="preview.isPending.value || backups.isPending.value"
    :error="preview.error.value"
    @retry="preview.refetch()"
  >
    <div class="flex flex-col gap-4">
      <SettingsSection :title="t('cryptoTrack.wipe.backupTitle')">
        <template #actions>
          <Button variant="outline" :loading="makeBackup.isPending.value" @click="makeBackup.mutate()">
            {{ t('cryptoTrack.wipe.backupMake') }}
          </Button>
        </template>
        <div class="flex flex-col gap-1" data-tour="crypto-wipe-backup">
          <p class="font-medium">
            {{ latest ? t('cryptoTrack.wipe.backupAt', { date: formatDateTime(latest.modified_at) }) : t('cryptoTrack.wipe.backupNone') }}
          </p>
          <p v-if="latest" class="text-sm text-muted-foreground">{{ latestLine }}</p>
          <p class="text-sm text-muted-foreground">{{ t('cryptoTrack.wipe.backupHint') }}</p>
        </div>
      </SettingsSection>

      <Card class="border-destructive/40">
        <template #header>
          <h3 class="heading-card text-destructive">{{ t('cryptoTrack.wipe.title') }}</h3>
        </template>
        <div class="flex flex-col gap-3 pt-2 text-sm" data-tour="crypto-wipe">
          <p>{{ t('cryptoTrack.wipeEntry.removes') }}</p>
          <p>{{ t('cryptoTrack.wipeEntry.stays') }}</p>
          <p>{{ t('cryptoTrack.wipeEntry.level') }}</p>
          <p v-if="preview.data.value?.empty" class="text-muted-foreground">{{ t('cryptoTrack.wipe.empty') }}</p>
          <div>
            <Button variant="destructive" :disabled="preview.data.value?.empty !== false" @click="askOpen = true">
              {{ t('cryptoTrack.wipeEntry.action') }}
            </Button>
          </div>
        </div>
      </Card>

      <div class="flex flex-col gap-1 rounded-lg bg-muted p-4 text-sm">
        <h3 class="font-medium text-muted-foreground">{{ t('cryptoTrack.wipeEntry.askTitle') }}</h3>
        <p>{{ t('cryptoTrack.wipeEntry.askBody') }}</p>
      </div>
    </div>
  </SectionState>

  <CryptoWipeDialog
    v-if="preview.data.value"
    v-model:open="askOpen"
    :preview="preview.data.value"
    :pending="wipe.isPending.value"
    @confirm="wipe.mutate(undefined, { onSuccess: () => (askOpen = false) })"
  />
</template>
