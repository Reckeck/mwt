<script setup lang="ts">
import { TriangleAlert } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'

/**
 * `Changed on another device`: the record behind an open edit form was changed by a sync.
 * `Refresh` loads the other device's values into the untouched fields and keeps the user's
 * edits. It never blocks saving.
 */
const emit = defineEmits<{ refresh: [] }>()
const { t } = useI18n()
</script>

<template>
  <Alert class="border-warning/40 bg-warning/10 text-foreground" data-sync-changed>
    <TriangleAlert class="text-warning" />
    <div class="flex flex-col items-start gap-2 md:flex-row md:items-center md:justify-between md:gap-4">
      <div class="min-w-0">
        <AlertTitle class="line-clamp-none">{{ t('sync.changed.title') }}</AlertTitle>
        <AlertDescription class="text-foreground/80">{{ t('sync.changed.body') }}</AlertDescription>
      </div>
      <Button type="button" variant="outline" size="sm" class="shrink-0" @click="emit('refresh')">
        {{ t('sync.changed.refresh') }}
      </Button>
    </div>
  </Alert>
</template>
