<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Check, ExternalLink, RotateCcw, Undo2 } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'

import type { VaultJsonStatus } from '../../model/use-vault-json-editor'

/** Where the draft stands, and the round trip through an external editor. */
const props = defineProps<{ status: VaultJsonStatus; loading: boolean; hasScratch: boolean }>()
const emit = defineEmits<{ openInEditor: []; reloadFromEditor: []; reset: [] }>()

const { t } = useI18n()
</script>

<template>
  <div class="flex flex-wrap items-center gap-2">
    <Badge :variant="props.status === 'saved' ? 'default' : 'secondary'">
      <span class="flex items-center gap-1">
        <Check v-if="props.status === 'saved'" class="size-3" />
        {{ t(`settings.dev.status.${props.status}`) }}
      </span>
    </Badge>
    <span class="flex-1" />
    <Button variant="ghost" size="sm" :disabled="props.loading" @click="emit('openInEditor')">
      <ExternalLink class="size-4" />
      {{ t('settings.dev.openInEditor') }}
    </Button>
    <Button
      variant="ghost"
      size="sm"
      :disabled="props.loading || !props.hasScratch"
      :title="t('settings.dev.reloadHint')"
      @click="emit('reloadFromEditor')"
    >
      <Undo2 class="size-4" />
      {{ t('settings.dev.reloadFromEditor') }}
    </Button>
    <Button variant="ghost" size="sm" :disabled="props.loading" @click="emit('reset')">
      <RotateCcw class="size-4" />
      {{ t('settings.dev.resetDraft') }}
    </Button>
  </div>
</template>
