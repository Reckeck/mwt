<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Check, CircleAlert } from 'lucide-vue-next'

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'

/** Step 5: what level 3 turned on. The Bybit key is typed again afterwards: it is not held in memory during the move. */
const props = defineProps<{ minutes: number | null; backupPath: string }>()
const { t } = useI18n()
</script>

<template>
  <div class="grid gap-3">
    <Alert>
      <Check />
      <AlertTitle>{{ t('securityRaise.done.title') }}</AlertTitle>
      <AlertDescription>{{ t('securityRaise.done.text') }}</AlertDescription>
    </Alert>
    <ul class="grid gap-2 text-sm">
      <li class="flex items-start gap-2"><Check class="mt-0.5 size-4 shrink-0 text-success" aria-hidden="true" />{{ t('securityRaise.done.exchanges') }}</li>
      <li v-if="props.minutes !== null" class="flex items-start gap-2">
        <Check class="mt-0.5 size-4 shrink-0 text-success" aria-hidden="true" />{{ t('securityRaise.done.autolock', { minutes: props.minutes }) }}
      </li>
      <li class="flex items-start gap-2">
        <Check class="mt-0.5 size-4 shrink-0 text-success" aria-hidden="true" />
        <span class="min-w-0 break-words">
          {{ props.backupPath ? t('securityRaise.done.backup', { path: props.backupPath }) : t('securityRaise.done.backupNext') }}
        </span>
      </li>
      <li class="flex items-start gap-2 text-muted-foreground">
        <CircleAlert class="mt-0.5 size-4 shrink-0" aria-hidden="true" />{{ t('securityRaise.done.reenter') }}
      </li>
    </ul>
  </div>
</template>
