<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { useI18n } from 'vue-i18n'
import { Loader2Icon } from 'lucide-vue-next'
import { cn } from '@/shared/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
}>()

const { t } = useI18n()
</script>

<template>
  <Loader2Icon
    role="status"
    :aria-label="t('ui.loading')"
    :class="cn('size-4 animate-spin', props.class)"
  />
</template>
