<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RotateCcw } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

/** Full-New-Sandbox 3: one question — what goes, and that the real vaults stay. */
defineProps<{ loading: boolean }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ confirm: [] }>()
const { t } = useI18n()
</script>

<template>
  <Dialog v-model:open="open" :title="t('sandbox.resetTitle')" size="confirm">
    <div class="grid gap-2 text-sm">
      <p>{{ t('sandbox.resetBody') }}</p>
      <p class="text-muted-foreground">{{ t('sandbox.resetReal') }}</p>
    </div>
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button :loading="loading" data-sandbox-reset-confirm @click="emit('confirm')">
        <RotateCcw class="size-4" aria-hidden="true" />
        {{ t('sandbox.reset') }}
      </Button>
    </template>
  </Dialog>
</template>
