<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

/** «Stop the move?» before the re-encryption: nothing is lost, the question only says what stays. */
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ stop: [] }>()
const { t } = useI18n()
</script>

<template>
  <Dialog
    v-model:open="open"
    overlay="security-raise-interrupt"
    size="sm"
    :title="t('securityRaise.interrupt.title')"
    :description="t('securityRaise.interrupt.text')"
  >
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('securityRaise.interrupt.stay') }}</Button>
      <Button variant="destructive" data-raise-stop @click="emit('stop')">{{ t('securityRaise.interrupt.stop') }}</Button>
    </template>
  </Dialog>
</template>
