<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Card } from '@/shared/ui/card'
import { EntityLink } from '@/shared/ui/entity-link'
import { cn } from '@/shared/lib/utils'

import { HEALTH_DOT } from '../capital-health.constants'
import { reasonStore, reasonText } from '../lib/reason-text'

/** The traffic light: one colour for the whole checkup, then each reason with its first step. */
const props = defineProps<{ report: HealthReport; names: Record<string, string> }>()
const { t } = useI18n()

const MARK: Record<HealthColor, string> = {
  red: 'text-destructive',
  yellow: 'text-warning',
  green: 'text-muted-foreground',
  no_data: 'text-muted-foreground',
}

const reasons = computed(() =>
  props.report.reasons.map((reason) => ({
    reason,
    text: reasonText(reason, props.report.currency, t),
    store: reasonStore(reason),
  })),
)
</script>

<template>
  <Card data-tour="health-light">
    <div class="flex items-center gap-3">
      <span :class="cn('size-4 shrink-0 rounded-full', HEALTH_DOT[props.report.color])" aria-hidden="true" />
      <div class="min-w-0">
        <p class="font-medium">{{ t(`health.color.${props.report.color}`) }}</p>
        <p class="text-sm text-muted-foreground">{{ t(`health.colorHint.${props.report.color}`) }}</p>
      </div>
    </div>
    <ul v-if="reasons.length" class="mt-4 grid gap-3">
      <li v-for="(row, index) in reasons" :key="index" class="grid gap-0.5 border-l-2 border-border pl-3 text-sm">
        <p :class="MARK[row.reason.level]">
          {{ row.text.what }}
          <EntityLink
            v-if="row.store"
            :target="{ kind: 'store', id: row.store }"
            :label="props.names[row.store] ?? ''"
          />
        </p>
        <p class="text-muted-foreground">{{ row.text.step }}</p>
      </li>
    </ul>
  </Card>
</template>
