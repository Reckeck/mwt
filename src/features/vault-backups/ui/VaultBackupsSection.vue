<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { Braces } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'
import { formatDateTime } from '@/shared/lib/format'
import { settingsQuery } from '@/shared/lib/settings-link'

import RestoreBackupDialog from './RestoreBackupDialog.vue'
import { useVaultBackups } from '../model/useVaultBackups'

const { t } = useI18n()
const router = useRouter()
const { backups, create, restore } = useVaultBackups()

const restoreOpen = ref(false)
const latest = computed(() => backups.data.value?.[0] ?? null)
const empty = computed(() => backups.data.value?.length === 0)
/** The JSON viewer lives in the developer group, which exists only in a development build. */
const hasJsonViewer = import.meta.env.DEV

function openJson() {
  void router.push({ query: settingsQuery(router.currentRoute.value.query, 'dev', 'devtools') })
}
</script>

<template>
  <SectionState :loading="backups.isPending.value" :error="backups.error.value" skeleton="figure" compact @retry="backups.refetch()">
    <div class="flex flex-wrap items-center gap-2">
      <Button size="sm" variant="outline" :loading="create.isPending.value" @click="create.mutate()">
        {{ t('vaultBackups.now') }}
      </Button>
      <Button size="sm" variant="outline" :disabled="empty || restore.isPending.value" @click="restoreOpen = true">
        {{ t('vaultBackups.restore') }}
      </Button>
      <Button v-if="hasJsonViewer" size="sm" variant="outline" @click="openJson">
        <Braces class="size-4" />
        {{ t('vaultBackups.json') }}
      </Button>
    </div>
    <p class="text-xs text-muted-foreground" data-vault-backup-last>
      <template v-if="latest">{{ t('vaultBackups.last') }}: {{ latest.name }} · {{ formatDateTime(latest.modified_at) }}</template>
      <template v-else>{{ t('vaultBackups.none') }}</template>
    </p>
  </SectionState>
  <RestoreBackupDialog
    v-model:open="restoreOpen"
    :backups="backups.data.value ?? []"
    :pending="restore.isPending.value"
    @restore="(name) => restore.mutate(name, { onSuccess: () => (restoreOpen = false) })"
  />
</template>
