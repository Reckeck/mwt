<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { CircleAlert } from 'lucide-vue-next'

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'

/** After a failed save: what is still missing, in one place above the save button. The messages stay under the fields. */
const props = defineProps<{ labels: string[] }>()
const { t } = useI18n()
</script>

<template>
  <Alert v-if="props.labels.length" variant="destructive" data-quick-missing>
    <CircleAlert />
    <AlertTitle>{{ t('quickAdd.missingTitle') }}</AlertTitle>
    <AlertDescription>{{ props.labels.join(', ') }}</AlertDescription>
  </Alert>
</template>
