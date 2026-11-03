<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useAppUpdate } from '@/features/app-update'
import { formatDateTime } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { SettingsSection } from '@/shared/ui/settings-section'
import { SwitchRow } from '@/shared/ui/switch-row'

/**
 * The app's own updates: version, what the last check found and what to do
 * about it. The status block keeps its height in every phase (rule 17).
 */
const { t } = useI18n()
const { status, view, checkNow, restart, autoUpdate, setAuto, run } = useAppUpdate()

const checking = computed(() => view.value.kind === 'checking' || checkNow.isPending.value)
const channelLabel = computed(() => (status.data.value?.channel === 'dev' ? t('update.channel.dev') : t('update.channel.prod')))
const lastCheck = computed(() => (status.data.value?.last_check ? formatDateTime(status.data.value.last_check) : null))

const line = computed(() => {
  const current = view.value
  switch (current.kind) {
    case 'checking':
      return t('update.status.checking')
    case 'upToDate':
      return lastCheck.value ? t('update.status.upToDateAt', { time: lastCheck.value }) : t('update.status.upToDate')
    case 'downloading':
      return t('update.status.downloading', { percent: current.percent })
    case 'ready':
      return t('update.status.ready', { version: current.version })
    case 'available':
      return t('update.status.available', { version: current.version })
    case 'error':
      return t('update.status.error', { message: current.message })
    default:
      return t('update.status.idle')
  }
})
const actionLabel = computed(() => {
  switch (view.value.action) {
    case 'restart':
      return t('update.action.restart')
    case 'openApk':
      return t('update.action.downloadApk')
    case 'openRelease':
      return t('update.action.openRelease')
    default:
      return null
  }
})
</script>

<template>
  <SettingsSection :title="t('settingsShell.section.updates')" :description="t('update.hint')">
    <div class="grid gap-3" data-tour="settings-updates">
      <div class="flex flex-wrap items-center gap-2 text-sm">
        <span class="text-muted-foreground">{{ t('update.version') }}</span>
        <span class="font-medium tabular-nums">{{ status.data.value?.current_version ?? '-' }}</span>
        <Badge variant="outline">{{ channelLabel }}</Badge>
      </div>
      <!-- One block for every phase: the line, a bar that is always there, the notes in a reserved row. -->
      <div class="grid min-h-24 content-start gap-2">
        <p class="text-sm" :class="view.kind === 'error' ? 'text-destructive' : view.kind === 'idle' ? 'text-muted-foreground' : ''" aria-live="polite">
          {{ line }}
        </p>
        <ProgressBar :value="view.kind === 'downloading' ? view.percent : 0" :label="t('update.progress')" size="sm" />
        <p class="line-clamp-3 min-h-4 text-xs text-muted-foreground">
          {{ view.action === 'packageManager' ? t('update.status.viaPackageManager') : (view.notes ?? '') }}
        </p>
      </div>
      <div class="flex flex-wrap gap-2">
        <Button :disabled="!actionLabel" :loading="restart.isPending.value" @click="run(view.action, view.url)">
          {{ actionLabel ?? t('update.action.none') }}
        </Button>
        <Button variant="outline" :disabled="checking" :loading="checking" @click="checkNow.mutate()">
          {{ t('update.action.check') }}
        </Button>
      </div>
      <SwitchRow
        plain
        id="auto-update"
        :label="t('update.auto')"
        :hint="t('update.autoHint')"
        :model-value="autoUpdate"
        :disabled="setAuto.isPending.value"
        @update:model-value="(value: boolean) => setAuto.mutate(value)"
      />
    </div>
  </SettingsSection>
</template>
