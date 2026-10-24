<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Check, CircleAlert, FolderOpen, Link2, ShieldOff, Trash2, X } from 'lucide-vue-next'
import { computed } from 'vue'

import { Button } from '@/shared/ui/button'
import { Badge } from '@/shared/ui/badge'
import { formatDayMonthShort } from '@/shared/lib/format'
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

/** «сегодня» or «12 окт»; empty when the history has no date. */
function when(recent: RecentVault): string {
  const seconds = Number(recent.last_opened)
  if (!Number.isFinite(seconds) || seconds <= 0) return ''
  const opened = new Date(seconds * 1000)
  if (Math.floor(Date.now() / 1000 / SECONDS_PER_DAY) === Math.floor(seconds / SECONDS_PER_DAY)) return t('vaultForget.today')
  return formatDayMonthShort(opened.toISOString())
}

/** Folder and when it was opened; a vault in the app's folder has no folder worth showing. */
function detail(recent: RecentVault): string {
  const where = recent.in_app_folder ? t('vaultDelete.inApp') : shortDir(recent.path, props.sheet)
  if (!recent.exists) return t('auth.missingDetail', { path: where || recent.path })
  return [where, when(recent)].filter(Boolean).join(' · ')
}

const hasForgettable = computed(() => vault.recents.some((recent: RecentVault) => !recent.in_app_folder))
const hasErasable = computed(() => vault.recents.some((recent: RecentVault) => recent.in_app_folder))
/** «Clear history» keeps the vault in the field and what is stored in the app. */
const clearable = computed(
  () => vault.recents.filter((recent: RecentVault) => !recent.in_app_folder && recent.path !== props.selected).length,
)
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
            <Badge v-if="!recent.encrypted" variant="warning" class="shrink-0 gap-1 font-normal" data-recents-plain>
              <ShieldOff class="size-3" />
              {{ t('vaultForget.plain') }}
            </Badge>
          </span>
          <span :class="cn('block truncate text-xs', recent.exists ? 'text-muted-foreground' : 'text-destructive')">
            {{ detail(recent) }}
          </span>
        </span>
      </Button>
      <!-- In the app's own folder the list is the files: nothing to take out, only to erase. -->
      <Button
        :variant="sheet ? 'outline' : 'ghost'"
        :size="sheet ? 'icon' : 'icon-xs'"
        :class="cn('shrink-0 text-muted-foreground', sheet && 'size-10', recent.in_app_folder && 'hover:text-destructive')"
        :aria-label="t(recent.in_app_folder ? 'vaultForget.deleteRow' : 'vaultForget.remove')"
        :title="t(recent.in_app_folder ? 'vaultForget.deleteRow' : 'vaultForget.remove')"
        :data-recents-delete="recent.in_app_folder || undefined"
        :data-recents-forget="!recent.in_app_folder || undefined"
        @click="recent.in_app_folder ? emit('remove', recent) : emit('forget', recent)"
      >
        <component :is="recent.in_app_folder ? Trash2 : X" :class="sheet ? 'size-4' : 'size-3.5'" />
      </Button>
    </div>

    <div v-if="!sheet || clearable || hasForgettable || hasErasable" class="mt-1 grid gap-0.5 border-t border-border pt-1">
      <Button v-if="!sheet" variant="ghost" class="min-h-10 justify-start gap-2" data-recents-other @click="emit('other')">
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
      <p v-if="hasForgettable" class="px-2.5 pt-1 pb-1.5 text-xs text-muted-foreground">
        {{ t(sheet ? 'vaultForget.hintSheet' : 'vaultForget.hint') }}
      </p>
      <p v-if="hasErasable" class="px-2.5 pt-1 pb-1.5 text-xs text-muted-foreground">{{ t('vaultForget.hintApp') }}</p>
    </div>
  </div>
</template>
