<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Badge } from '@/shared/ui/badge'
import { outcomeDetail, outcomeKey } from '@/shared/lib/workflow'

/** How a run ended, with the reason on hover. */
const props = defineProps<{ outcome: RunOutcome; dryRun?: boolean }>()

const { t } = useI18n()
const key = computed(() => outcomeKey(props.outcome))
const variant = computed(() => (key.value === 'failed' ? 'destructive' : key.value === 'done' ? 'secondary' : 'outline'))
const title = computed(() => {
  const detail = outcomeDetail(props.outcome)
  return key.value === 'dropped' ? t(`workflow.dropped.${detail}`) : detail || undefined
})
</script>

<template>
  <Badge :variant="variant" :title="title">
    {{ t(`workflow.outcome.${key}`) }}<template v-if="props.dryRun"> · {{ t('workflow.outcome.dryRun') }}</template>
  </Badge>
</template>
