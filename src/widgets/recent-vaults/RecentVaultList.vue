<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Check, CircleAlert, FolderOpen, Link2, Trash2, X } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { formatDate } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { useVaultStore } from '@/shared/store/vault'

import { shortDir } from './lib/short-path'

const props = defineProps<{
  /** Path of the vault the field shows. */
  selected: string | null
  /** Bottom sheet on a phone: framed «×», no group title, «Another file» sits below the list. */
  sheet?: boolean
}>()
const emit = defineEmits<{
  pick: [path: string]
  forget: [recent: RecentVault]
  clear: []
  other: []
}>()

const { t } = useI18n()
const vault = useVaultStore()

function title(recent: RecentVault): string {
  return recent.display_name || recent.path
}

/** Folder and when it was opened; in app storage the path tells nothing. */
function detail(recent: RecentVault): string {
  const where = vault.appStorage ? '' : shortDir(recent.path, props.sheet)
  if (!recent.exists) return t('auth.missingDetail', { path: where || recent.path })
  const opened = new Date(Number(recent.last_opened) * 1000)
  const when = Number.isNaN(opened.getTime()) ? '' : formatDate(opened.toISOString())
  return [where, when].filter(Boolean).join(' · ')
}
</script>

<template>
  <div class="grid gap-1" data-recents-list>
    <p v-if="!sheet" class="px-2.5 pt-1.5 pb-1 text-xs text-muted-foreground">{{ t('vaultForget.group') }}</p>
    <div v-for="recent in vault.recents" :key="recent.path" class="flex items-center gap-1" data-recents-row>
      <Button
        variant="ghost"
        size="block"
        :class="cn('min-h-11 min-w-0 flex-1 gap-2 px-2.5 py-2', selected === recent.path && 'bg-muted')"
        @click="emit('pick', recent.path)"
      >
        <span class="flex size-4 shrink-0 items-center justify-center">
          <Check v-if="selected === recent.path" class="size-4 text-primary" :aria-label="t('vaultForget.current')" />
          <CircleAlert v-else-if="!recent.exists" class="size-4 text-destructive" aria-hidden="true" />
        </span>
        <span class="min-w-0 flex-1">
          <span class="flex items-center gap-1.5 text-sm font-medium">
            <span class="truncate">{{ title(recent) }}</span>
            <Link2 v-if="recent.linked" class="size-3.5 shrink-0 text-muted-foreground" :aria-label="t('vaultLink.linkedBadge')" />
          </span>
          <span :class="cn('block truncate text-xs', recent.exists ? 'text-muted-foreground' : 'text-destructive')">
            {{ detail(recent) }}
          </span>
        </span>
      </Button>
      <!-- App storage lists every vault on the device: nothing to take out of a list. -->
      <Button
        v-if="!vault.appStorage"
        :variant="sheet ? 'outline' : 'ghost'"
        :size="sheet ? 'icon' : 'icon-xs'"
        :class="cn('shrink-0 text-muted-foreground', sheet && 'size-10')"
        :aria-label="t('vaultForget.remove')"
        :title="t('vaultForget.remove')"
        data-recents-forget
        @click="emit('forget', recent)"
      >
        <X :class="sheet ? 'size-4' : 'size-3.5'" />
      </Button>
    </div>

    <div v-if="!vault.appStorage || !sheet" class="mt-1 grid gap-0.5 border-t border-border pt-1">
      <Button v-if="!sheet" variant="ghost" class="min-h-10 justify-start gap-2" data-recents-other @click="emit('other')">
        <FolderOpen class="size-4" />
        {{ t(vault.appStorage ? 'auth.importFile' : 'vaultForget.otherFile') }}
      </Button>
      <Button
        v-if="!vault.appStorage && vault.recents.length > 1"
        variant="ghost"
        class="min-h-10 justify-start gap-2 text-muted-foreground"
        data-recents-clear
        @click="emit('clear')"
      >
        <Trash2 class="size-4" />
        {{ t('vaultForget.clear') }}
      </Button>
      <p v-if="!vault.appStorage" class="px-2.5 pt-1 pb-1.5 text-xs text-muted-foreground">
        {{ t(sheet ? 'vaultForget.hintSheet' : 'vaultForget.hint') }}
      </p>
    </div>
  </div>
</template>
