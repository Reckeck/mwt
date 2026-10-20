<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RefreshCw } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Spinner } from '@/shared/ui/spinner'
import type { AppError } from '@/shared/api/ipc'

/** What covers an empty chart: loading, a failed load with «Обновить», or no data. */
const props = defineProps<{ loading: boolean; error: AppError | null }>()
const emit = defineEmits<{ retry: [] }>()

const { t } = useI18n()
</script>

<template>
  <div class="absolute inset-0 z-sticky flex flex-col items-center justify-center gap-3 bg-card px-6 text-center text-sm">
    <template v-if="props.loading">
      <Spinner class="size-5 text-muted-foreground" />
      <span class="text-muted-foreground">{{ t('trading.loading') }}</span>
    </template>
    <template v-else-if="props.error">
      <span class="text-destructive">{{ props.error.message }}</span>
      <Button variant="outline" size="sm" @click="emit('retry')">
        <RefreshCw class="size-4" />
        {{ t('actions.refresh') }}
      </Button>
    </template>
    <span v-else class="text-muted-foreground">{{ t('trading.empty') }}</span>
  </div>
</template>
