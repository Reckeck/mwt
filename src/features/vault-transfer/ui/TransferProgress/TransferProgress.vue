<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatQuantity } from '@/shared/lib/format'

import type { TransferProgressProps } from './transfer-progress.types'
import { ProgressBar } from '@/shared/ui/progress-bar'

/** How much of the vault has crossed: a bar and the size. */
const props = defineProps<TransferProgressProps>()
const { t } = useI18n()

const percent = computed(() => (props.total > 0 ? Math.min(100, Math.round((props.done / props.total) * 100)) : 0))
const megabytes = (bytes: number) => formatQuantity(bytes / 1_048_576, 1)
</script>

<template>
  <div class="grid gap-2" aria-live="polite">
    <ProgressBar :value="percent" :label="t('transfer.progressLabel')" />
    <p class="text-xs text-muted-foreground tabular-nums">
      {{ t('transfer.progress', { done: megabytes(done), total: megabytes(total), percent }) }}
    </p>
  </div>
</template>
