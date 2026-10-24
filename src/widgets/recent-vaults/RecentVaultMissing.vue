<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { CircleAlert, FolderSearch, X } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'

import { shortDir } from './lib/short-path'

const props = defineProps<{
  recent: RecentVault
  /** Phone: a card with the name instead of a line under the field. */
  card?: boolean
  finding?: boolean
}>()
const emit = defineEmits<{ find: []; forget: [] }>()

const { t } = useI18n()

function name(): string {
  return props.recent.display_name || props.recent.path
}
</script>

<template>
  <!-- The file of the chosen history row is gone (Full-New-Recents 3): why, and the two ways out. -->
  <Card v-if="card" class="border-destructive/40" data-recents-missing>
    <div class="grid gap-2">
      <div class="flex items-center gap-2">
        <CircleAlert class="size-4 shrink-0 text-destructive" aria-hidden="true" />
        <p class="min-w-0 flex-1 truncate text-sm font-semibold">{{ t('vaultForget.missingTitle', { name: name() }) }}</p>
      </div>
      <p class="text-sm text-muted-foreground">{{ t('vaultForget.missingWhere', { dir: shortDir(recent.path, true) }) }}</p>
      <div class="flex gap-2">
        <Button variant="outline" class="min-h-11 flex-1 gap-2" :loading="finding" data-recents-find @click="emit('find')">
          <FolderSearch class="size-4" />
          {{ t('vaultForget.find') }}
        </Button>
        <Button variant="outline" class="min-h-11 flex-1 gap-2" data-recents-drop @click="emit('forget')">
          <X class="size-4" />
          {{ t('vaultForget.removeShort') }}
        </Button>
      </div>
    </div>
  </Card>
  <div v-else class="grid gap-2" data-recents-missing>
    <p role="alert" class="text-xs text-destructive">{{ t('vaultForget.missingError') }}</p>
    <div class="flex flex-wrap gap-2">
      <Button variant="outline" size="sm" class="gap-1.5" :loading="finding" data-recents-find @click="emit('find')">
        <FolderSearch class="size-4" />
        {{ t('vaultForget.find') }}
      </Button>
      <Button variant="outline" size="sm" class="gap-1.5" data-recents-drop @click="emit('forget')">
        <X class="size-4" />
        {{ t('vaultForget.remove') }}
      </Button>
    </div>
  </div>
</template>
