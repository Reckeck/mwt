<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import type { ConfirmDialogProps } from './confirm-dialog.types'

/**
 * A question with the consequence spelled out, for a destructive action started from a menu (where
 * `ConfirmButton` has no place to stand). The caller closes it when the action is done.
 */
const props = withDefaults(defineProps<ConfirmDialogProps>(), { confirmLabel: undefined, pending: false, overlay: 'confirm' })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ confirm: [] }>()
const { t } = useI18n()
</script>

<template>
  <Dialog v-model:open="open" :overlay="props.overlay" size="sm" :title="props.title" :description="props.description">
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button variant="destructive" :loading="props.pending" :disabled="props.pending" @click="emit('confirm')">
        {{ props.confirmLabel ?? t('actions.delete') }}
      </Button>
    </template>
  </Dialog>
</template>
