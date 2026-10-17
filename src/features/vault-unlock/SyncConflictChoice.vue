<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { FileWarning } from 'lucide-vue-next'

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'

defineProps<{ busy: boolean }>()
const emit = defineEmits<{ keep: [side: ConflictSide] }>()

const { t } = useI18n()
</script>

<template>
  <!-- The vault and its external copy both changed since the last sync. -->
  <Alert variant="destructive" role="alert">
    <FileWarning aria-hidden="true" />
    <AlertTitle>{{ t('vaultLink.conflictTitle') }}</AlertTitle>
    <AlertDescription class="grid gap-2">
      <p>{{ t('vaultLink.conflictBody') }}</p>
      <div class="flex flex-wrap gap-2">
        <Button size="sm" variant="outline" :disabled="busy" @click="emit('keep', 'local')">
          {{ t('vaultLink.keepLocal') }}
        </Button>
        <Button size="sm" variant="outline" :disabled="busy" @click="emit('keep', 'remote')">
          {{ t('vaultLink.keepRemote') }}
        </Button>
      </div>
    </AlertDescription>
  </Alert>
</template>
