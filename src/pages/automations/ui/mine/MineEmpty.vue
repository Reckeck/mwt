<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ChevronRight } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { useIsPhone } from '@/shared/lib/phone'

import { MINE_STARTERS, type MineStarter } from '../../lib/mine-view'

/**
 * Nothing fires on its own yet (Auto-1 · 2): what will be here and four
 * ready-made starts. «Настроить →» opens the wizard with that start.
 */
const emit = defineEmits<{ pick: [starter: MineStarter] }>()
const { t } = useI18n()
const phone = useIsPhone()
</script>

<template>
  <div v-if="!phone" class="flex flex-col items-start gap-3 rounded-xl border border-dashed border-border px-5 py-5" data-mine-empty data-tour="automations-starts">
    <h2 class="text-base font-semibold">{{ t('automations.mine.empty.title') }}</h2>
    <p class="text-caption text-muted-foreground">{{ t('automations.mine.empty.text') }}</p>
    <div class="grid w-full grid-cols-4 gap-3">
      <Button
        v-for="starter in MINE_STARTERS"
        :key="starter"
        variant="outline"
        class="h-auto flex-col items-start justify-start gap-1 rounded-lg bg-card p-2.5 text-left font-normal whitespace-normal hover:border-primary"
        :data-mine-starter="starter"
        @click="emit('pick', starter)"
      >
        <span class="text-caption text-muted-foreground">{{ t(`automations.mine.starter.${starter}.title`) }}</span>
        <span class="text-xs text-muted-foreground">{{ t(`automations.mine.starter.${starter}.text`) }}</span>
        <span class="text-xs text-primary">{{ t('automations.mine.empty.setUp') }} →</span>
      </Button>
    </div>
  </div>
  <div v-else class="flex flex-col gap-3" data-mine-empty data-tour="automations-starts">
    <p class="text-sm text-muted-foreground">{{ t('automations.mine.empty.phone') }}</p>
    <ul class="rounded-xl border border-border bg-card px-3 shadow-sm">
      <li v-for="starter in MINE_STARTERS" :key="starter" class="border-b border-border last:border-b-0">
        <Button
          variant="ghost"
          class="h-auto min-h-13 w-full justify-start gap-3 rounded-none px-0 py-1.5 text-left font-normal hover:bg-transparent"
          :data-mine-starter="starter"
          @click="emit('pick', starter)"
        >
          <span class="flex min-w-0 flex-1 flex-col">
            <span class="truncate font-medium">{{ t(`automations.mine.starter.${starter}.title`) }}</span>
            <span class="truncate text-xs text-muted-foreground">{{ t(`automations.mine.starter.${starter}.text`) }}</span>
          </span>
          <ChevronRight class="size-4 shrink-0 text-muted-foreground" />
        </Button>
      </li>
    </ul>
  </div>
</template>
