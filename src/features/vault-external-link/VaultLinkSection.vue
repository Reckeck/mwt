<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { FilePlus2, FolderOpen } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'
import { formatDateTime } from '@/shared/lib/format'

import { useVaultLink } from './useVaultLink'

/** The body of the «Копия во внешнем файле» card: whether it is on, where, and the buttons. */
const { t } = useI18n()
const { status, link, unlink } = useVaultLink()

const current = computed(() => status.data.value)
const busy = computed(() => link.isPending.value || unlink.isPending.value)
</script>

<template>
  <SectionState
    :loading="status.isPending.value"
    :error="status.error.value"
    skeleton="figure"
    compact
    @retry="status.refetch()"
  >
    <div class="grid gap-1 text-sm" data-vault-link-status>
      <p v-if="current?.linked" class="min-w-0 break-words">
        <span class="font-medium text-success">{{ t('vaultLink.statusOn') }}</span>
        · {{ current.display_name }}
        <template v-if="current.last_synced_at && !current.last_error"> · {{ formatDateTime(current.last_synced_at) }}</template>
      </p>
      <p v-else class="font-medium text-muted-foreground">{{ t('vaultLink.statusOff') }}</p>
      <p v-if="current?.last_error" role="alert" class="text-xs text-destructive">
        {{ t('vaultLink.lastFailed') }}: {{ current.last_error }}
      </p>
    </div>

    <div class="flex flex-wrap items-center gap-2">
      <template v-if="current?.linked">
        <Button size="sm" variant="outline" :disabled="busy" :loading="link.isPending.value" @click="link.mutate(false)">
          {{ t('vaultLink.change') }}
        </Button>
        <Button size="sm" variant="outline" :disabled="busy" :loading="unlink.isPending.value" @click="unlink.mutate()">
          {{ t('vaultLink.turnOff') }}
        </Button>
      </template>
      <template v-else>
        <Button size="sm" variant="outline" class="gap-2" :disabled="busy" :loading="link.isPending.value && link.variables.value === false" @click="link.mutate(false)">
          <FilePlus2 class="size-4" />
          {{ t('vaultLink.createCopy') }}
        </Button>
        <Button
          size="sm"
          variant="outline"
          class="gap-2"
          :disabled="busy"
          :title="t('vaultLink.pickExistingHint')"
          :loading="link.isPending.value && link.variables.value === true"
          @click="link.mutate(true)"
        >
          <FolderOpen class="size-4" />
          {{ t('vaultLink.pickExisting') }}
        </Button>
      </template>
    </div>
  </SectionState>
</template>
