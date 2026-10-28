<script setup lang="ts">
import { Copy } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'

import { copyText } from '@/shared/lib/clipboard'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

import type { ReadonlyFieldProps } from './readonly-field.types'

/**
 * A caption and a value that look like a field but are not edited: a total or a share
 * the form computes. The row keeps the height of an input (and of a touch target on a
 * phone) whether or not the copy button is there, so nothing moves.
 */
const props = defineProps<ReadonlyFieldProps>()
const { t } = useI18n()
</script>

<template>
  <div class="grid gap-1.5" data-slot="readonly-field">
    <span class="text-sm font-medium leading-none">{{ props.label }}</span>
    <div
      :class="cn('flex h-10 items-center gap-2 rounded-md border border-input bg-muted/50 px-3 text-sm md:h-9')"
    >
      <span :class="cn('min-w-0 flex-1 truncate tabular-nums', !props.value && 'text-muted-foreground')">
        {{ props.value || '—' }}
      </span>
      <Button
        v-if="props.copyable"
        variant="ghost"
        size="icon-sm"
        class="-mr-2 shrink-0"
        :aria-label="t('ui.copy')"
        :disabled="!props.value"
        @click="copyText(props.value ?? '', props.copyKey)"
      >
        <Copy class="size-4" />
      </Button>
    </div>
  </div>
</template>
