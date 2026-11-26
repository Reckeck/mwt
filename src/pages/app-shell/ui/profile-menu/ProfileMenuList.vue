<script setup lang="ts">
import { Button } from '@/shared/ui/button'
import { Separator } from '@/shared/ui/separator'
import { cn } from '@/shared/lib/utils'

import type { ProfileEntry } from './use-profile-menu'

/** The rows of the profile menu, one list for the popover and the sheet: a row is a button, never hover-only. */
defineProps<{
  vaultName: string
  vaultEntries: ProfileEntry[]
  statusEntries: ProfileEntry[]
  appEntries: ProfileEntry[]
}>()
</script>

<template>
  <div class="grid gap-1" data-profile-menu>
    <p class="truncate px-2 pt-1 text-sm font-semibold" data-profile-vault>{{ vaultName }}</p>
    <template v-if="vaultEntries.length">
      <Button
        v-for="entry in vaultEntries"
        :key="entry.id"
        variant="ghost"
        :class="cn('h-auto min-h-11 w-full justify-start gap-3 px-2 py-1.5 text-left text-sm font-normal', entry.destructive && 'text-destructive hover:text-destructive')"
        :data-profile-item="entry.id"
        @click="entry.run()"
      >
        <component :is="entry.icon" class="size-4 shrink-0" />
        <span class="grid min-w-0 flex-1">
          <span class="truncate">{{ entry.label }}</span>
          <span v-if="entry.hint" class="truncate text-xs text-muted-foreground">{{ entry.hint }}</span>
        </span>
      </Button>
      <Separator />
    </template>
    <template v-for="(group, index) in [statusEntries, appEntries]" :key="index">
      <Button
        v-for="entry in group"
        :key="entry.id"
        variant="ghost"
        class="h-auto min-h-11 w-full justify-start gap-3 px-2 py-1.5 text-left text-sm font-normal"
        :data-profile-item="entry.id"
        @click="entry.run()"
      >
        <component :is="entry.icon" :class="cn('size-4 shrink-0', entry.iconClass ?? 'text-muted-foreground')" />
        <span class="grid min-w-0 flex-1">
          <span class="truncate">{{ entry.label }}</span>
          <span v-if="entry.hint" class="truncate text-xs text-muted-foreground">{{ entry.hint }}</span>
        </span>
        <span
          v-if="entry.count"
          class="shrink-0 rounded-full bg-destructive px-1.5 text-xs font-medium text-destructive-foreground tabular-nums"
          data-profile-count
        >{{ entry.count }}</span>
      </Button>
      <Separator v-if="index === 0" />
    </template>
  </div>
</template>
