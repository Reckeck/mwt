<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { FilePlus2, FolderOpen, Link2, Unlink } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { SectionState } from '@/shared/ui/section-state'
import { Spinner } from '@/shared/ui/spinner'
import { formatDateTime } from '@/shared/lib/format'

import { useVaultLink } from './useVaultLink'

const { t } = useI18n()
const { status, link, unlink } = useVaultLink()

const current = computed(() => status.data.value)
const busy = computed(() => link.isPending.value || unlink.isPending.value)
</script>

<template>
  <!-- A copy of the vault in a file of the user's choice, rewritten after every save. -->
  <section class="grid gap-2 rounded-md border border-border px-3 py-2">
    <div class="flex items-center gap-2">
      <Link2 class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <h4 class="text-sm font-medium">{{ t('vaultLink.title') }}</h4>
    </div>
    <p class="text-xs text-muted-foreground">{{ t('vaultLink.hint') }}</p>

    <SectionState
      :loading="status.isPending.value"
      :error="status.error.value"
      skeleton="figure"
      compact
      @retry="status.refetch()"
    >
      <div v-if="current?.linked" class="grid gap-1 text-sm">
        <p class="truncate">
          {{ t('vaultLink.target') }}: <span class="font-medium">{{ current.display_name }}</span>
        </p>
        <p v-if="current.last_error" role="alert" class="text-xs text-destructive">
          {{ t('vaultLink.lastFailed') }}: {{ current.last_error }}
        </p>
        <p v-else-if="current.last_synced_at" class="text-xs text-muted-foreground">
          {{ t('vaultLink.lastSynced') }}: {{ formatDateTime(current.last_synced_at) }}
        </p>
      </div>

      <div class="mt-1 flex flex-wrap items-center gap-2">
        <Button size="sm" variant="outline" class="gap-2" :disabled="busy" @click="link.mutate(false)">
          <Spinner v-if="link.isPending.value && link.variables.value === false" />
          <FilePlus2 v-else class="size-4" />
          {{ t('vaultLink.createCopy') }}
        </Button>
        <Button
          size="sm"
          variant="outline"
          class="gap-2"
          :disabled="busy"
          :title="t('vaultLink.pickExistingHint')"
          @click="link.mutate(true)"
        >
          <Spinner v-if="link.isPending.value && link.variables.value === true" />
          <FolderOpen v-else class="size-4" />
          {{ t('vaultLink.pickExisting') }}
        </Button>
        <ConfirmButton
          v-if="current?.linked"
          :label="t('vaultLink.unlink')"
          :confirm-label="t('vaultLink.unlinkConfirm')"
          :disabled="busy"
          @confirm="unlink.mutate()"
        >
          <Unlink class="size-3.5" />
        </ConfirmButton>
      </div>
    </SectionState>
  </section>
</template>
