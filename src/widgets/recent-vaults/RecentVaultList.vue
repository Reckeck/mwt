<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { CircleAlert, FolderOpen, Link2, Lock, ShieldOff, Trash2, X } from 'lucide-vue-next'
import { computed } from 'vue'

import { Button } from '@/shared/ui/button'
import { Badge } from '@/shared/ui/badge'
import { ListRow } from '@/shared/ui/list-row'
import { formatDayMonthShort } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { useVaultStore } from '@/shared/store/vault'

import { shortDir } from './lib/short-path'

const props = defineProps<{
  /** Path of the vault the card opens. */
  selected: string | null
}>()
const emit = defineEmits<{
  pick: [path: string]
  forget: [recent: RecentVault]
  remove: [recent: RecentVault]
  clear: []
  other: []
}>()

const { t } = useI18n()
const vault = useVaultStore()

function title(recent: RecentVault): string {
  return recent.display_name || recent.path
}

const SECONDS_PER_DAY = 86400

/** `today` or `Oct 12`; empty when the history has no date. */
function when(recent: RecentVault): string {
  const seconds = Number(recent.last_opened)
  if (!Number.isFinite(seconds) || seconds <= 0) return ''
  const opened = new Date(seconds * 1000)
  if (Math.floor(Date.now() / 1000 / SECONDS_PER_DAY) === Math.floor(seconds / SECONDS_PER_DAY)) return t('vaultForget.today')
  return formatDayMonthShort(opened.toISOString())
}

/** Folder and when it was opened; a vault in the app's folder has no folder worth showing. */
function detail(recent: RecentVault): string {
  const where = recent.in_app_folder ? t('vaultDelete.inApp') : shortDir(recent.path, true)
  if (!recent.exists) return t('auth.missingDetail', { path: where || recent.path })
  return [where, when(recent)].filter(Boolean).join(' · ')
}

const hasForgettable = computed(() => vault.recents.some((recent: RecentVault) => !recent.in_app_folder))
const hasErasable = computed(() => vault.recents.some((recent: RecentVault) => recent.in_app_folder))
/** «Clear history» keeps the vault in the card and what is stored in the app. */
const clearable = computed(
  () => vault.recents.filter((recent: RecentVault) => !recent.in_app_folder && recent.path !== props.selected).length,
)
</script>

<template>
  <div class="grid gap-1" data-recents-list>
    <div v-for="recent in vault.recents" :key="recent.path" class="flex items-center gap-1" data-recents-row>
      <Button
        variant="ghost"
        size="block"
        :aria-pressed="selected === recent.path"
        :class="cn('h-auto min-w-0 flex-1 px-2 py-0 font-normal', selected === recent.path && 'bg-muted')"
        data-recents-pick
        @click="emit('pick', recent.path)"
      >
        <ListRow :title="title(recent)" :subtitle="detail(recent)" :icon="recent.exists ? Lock : CircleAlert">
          <template #value>
            <span class="flex items-center gap-1.5">
              <Link2 v-if="recent.linked" class="size-3.5 text-muted-foreground" :aria-label="t('vaultLink.linkedBadge')" />
              <Badge v-if="!recent.encrypted" variant="warning" class="gap-1 font-normal" data-recents-plain>
                <ShieldOff class="size-3" />
                {{ t('vaultForget.plain') }}
              </Badge>
            </span>
          </template>
        </ListRow>
      </Button>
      <!-- In the app's own folder the list is the files: nothing to take out, only to erase. -->
      <Button
        variant="ghost"
        size="icon"
        :class="cn('shrink-0 text-muted-foreground', recent.in_app_folder && 'hover:text-destructive')"
        :aria-label="t(recent.in_app_folder ? 'vaultForget.deleteRow' : 'vaultForget.remove')"
        :title="t(recent.in_app_folder ? 'vaultForget.deleteRow' : 'vaultForget.remove')"
        :data-recents-delete="recent.in_app_folder || undefined"
        :data-recents-forget="!recent.in_app_folder || undefined"
        @click="recent.in_app_folder ? emit('remove', recent) : emit('forget', recent)"
      >
        <component :is="recent.in_app_folder ? Trash2 : X" class="size-4" />
      </Button>
    </div>

    <div class="mt-1 grid gap-0.5 border-t border-border pt-1">
      <Button variant="ghost" class="min-h-10 justify-start gap-2" data-recents-other @click="emit('other')">
        <FolderOpen class="size-4" />
        {{ t(vault.appStorage ? 'vaultForget.addFromFile' : 'vaultForget.otherFile') }}
      </Button>
      <Button
        v-if="clearable"
        variant="ghost"
        class="min-h-10 justify-start gap-2 text-muted-foreground"
        data-recents-clear
        @click="emit('clear')"
      >
        <Trash2 class="size-4" />
        {{ t('vaultForget.clear') }}
      </Button>
      <p v-if="hasForgettable" class="px-2.5 pt-1 pb-1.5 text-xs text-muted-foreground">{{ t('vaultForget.hintSheet') }}</p>
      <p v-if="hasErasable" class="px-2.5 pt-1 pb-1.5 text-xs text-muted-foreground">{{ t('vaultForget.hintApp') }}</p>
    </div>
  </div>
</template>
