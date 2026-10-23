<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { FileUp } from 'lucide-vue-next'

/**
 * The whole card lit up while a file is over it (Attach 3): what it lands on.
 * check-ui-ignore hand-empty: the dashed frame is the drop target, not an empty state
 */
const props = defineProps<{ ownerLabel: string }>()

const { t } = useI18n()
</script>

<template>
  <div
    class="pointer-events-none absolute inset-2 z-sticky flex flex-col items-center justify-center gap-2.5
      rounded-xl border-2 border-dashed border-primary bg-background p-6 text-center"
    data-attachment-drop
  >
    <span class="absolute inset-0 rounded-[inherit] bg-primary/8" />
    <FileUp class="relative size-6 text-primary" />
    <span class="relative text-lg font-semibold">{{ t('attachments.dropHere') }}</span>
    <span class="relative text-xs text-muted-foreground">{{ props.ownerLabel }}</span>
  </div>
</template>
