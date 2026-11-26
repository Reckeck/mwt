<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { CheckCircle2, CircleDashed, Clock, Hand, OctagonX, XCircle } from 'lucide-vue-next'

import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

/** A run in words, step by step, with the values each node gave. */
const props = defineProps<{ steps: TraceStep[] }>()
const emit = defineEmits<{ focus: [nodeId: string] }>()

const { t, locale } = useI18n()

const ICON = {
  ok: CheckCircle2,
  skipped: CircleDashed,
  queued: Hand,
  error: XCircle,
  stopped: OctagonX,
  waiting: Clock,
} as const
const COLOR: Record<StepStatus, string> = {
  ok: 'text-primary',
  skipped: 'text-muted-foreground',
  queued: 'text-muted-foreground',
  error: 'text-destructive',
  stopped: 'text-muted-foreground',
  waiting: 'text-muted-foreground',
}
</script>

<template>
  <ol v-if="props.steps.length" class="grid gap-1.5">
    <li v-for="(step, index) in props.steps" :key="index">
      <Button
        variant="ghost"
        size="block"
        class="flex w-full items-start gap-2 rounded-md px-2 py-1 text-left text-sm hover:bg-muted"
        @click="emit('focus', step.node_id)"
      >
        <component :is="ICON[step.status]" :class="cn('mt-0.5 size-4 shrink-0', COLOR[step.status])" />
        <span class="min-w-0 flex-1">
          <span class="block">{{ locale.startsWith('ru') ? step.text_ru : step.text_en }}</span>
          <span v-if="Object.keys(step.values).length" class="block truncate text-xs text-muted-foreground">
            {{
              Object.entries(step.values)
                .map(([port, value]) => `${port}: ${value}`)
                .join(' · ')
            }}
          </span>
        </span>
      </Button>
    </li>
  </ol>
  <p v-else class="text-sm text-muted-foreground">{{ t('workflow.trace.empty') }}</p>
</template>
