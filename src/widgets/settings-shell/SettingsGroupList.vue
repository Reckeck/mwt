<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ChevronRight } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { cn } from '@/shared/lib/utils'

import type { SettingsGroup } from './settings-shell.constants'

/** Groups of the left menu; on a phone the same list is the first screen. */
const props = defineProps<{ groups: SettingsGroup[]; activeId: string; list: boolean; file: string }>()
const emit = defineEmits<{ select: [id: string] }>()
const { t } = useI18n()
</script>

<template>
  <div class="flex flex-col gap-1 max-md:gap-4">
    <div class="flex flex-col gap-1 max-md:gap-0 max-md:rounded-xl max-md:border max-md:border-border max-md:bg-card max-md:px-3 max-md:shadow-sm">
      <Button
        v-for="group in props.groups"
        :key="group.id"
        variant="ghost"
        size="block"
        :class="cn(
          'h-12 gap-3 px-2.5 font-medium',
          'max-md:h-13 max-md:rounded-none max-md:border-b max-md:border-border max-md:px-0 max-md:py-0',
          'max-md:last:border-b-0 max-md:hover:bg-transparent',
          !props.list && props.activeId === group.id && 'bg-accent text-accent-foreground',
        )"
        :aria-current="!props.list && props.activeId === group.id ? 'page' : undefined"
        @click="emit('select', group.id)"
      >
        <span class="flex shrink-0 items-center justify-center text-muted-foreground max-md:size-9 max-md:rounded-[0.625rem] max-md:bg-muted">
          <component :is="group.icon" class="size-4" />
        </span>
        <span class="grid min-w-0 flex-1 text-left">
          <span class="truncate">{{ t(group.titleKey) }}</span>
          <span class="truncate text-xs font-normal text-muted-foreground">{{ t(group.hintKey) }}</span>
        </span>
        <ChevronRight class="size-4 shrink-0 text-muted-foreground md:hidden" aria-hidden="true" />
      </Button>
    </div>
    <p v-if="props.file" class="truncate text-center text-sm text-muted-foreground md:hidden">{{ t('settingsShell.footer', { app: t('app.title'), file: props.file }) }}</p>
  </div>
</template>
