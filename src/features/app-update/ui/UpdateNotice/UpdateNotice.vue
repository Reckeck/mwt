<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Download } from 'lucide-vue-next'

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'

import { noticeVersion } from '../../lib/update-view'
import { useAppUpdate } from '../../model/use-app-update'

/**
 * A new version is ready to install or can be fetched by hand. Non-blocking:
 * lives in the shell's banner stack, once per version, `Later` hides it for
 * the session.
 */
const { t } = useI18n()
const { view, restart, run } = useAppUpdate()
const dismissed = ref(new Set<string>())

// Up to date again: a later offer of the same version is news again.
watch(
  () => view.value.kind,
  (kind) => {
    if (kind === 'upToDate') dismissed.value = new Set()
  },
)

const version = computed(() => noticeVersion(view.value))
const shown = computed(() => version.value !== null && !dismissed.value.has(version.value))
const primaryLabel = computed(() => {
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

function later() {
  if (version.value) dismissed.value = new Set([...dismissed.value, version.value])
}
</script>

<template>
  <Alert v-if="shown" class="pointer-events-auto bg-card shadow-lg" data-testid="update-notice">
    <Download />
    <AlertTitle>{{ t(view.kind === 'ready' ? 'update.notice.readyTitle' : 'update.notice.availableTitle', { version }) }}</AlertTitle>
    <AlertDescription>
      <p>
        {{
          view.kind === 'ready'
            ? t('update.notice.readyBody')
            : view.action === 'packageManager'
              ? t('update.status.viaPackageManager')
              : t('update.notice.availableBody')
        }}
      </p>
      <div class="mt-2 flex flex-wrap gap-2">
        <Button v-if="primaryLabel" size="sm" :loading="restart.isPending.value" @click="run(view.action, view.url)">
          {{ primaryLabel }}
        </Button>
        <Button size="sm" variant="ghost" @click="later">{{ t('update.action.later') }}</Button>
      </div>
    </AlertDescription>
  </Alert>
</template>
