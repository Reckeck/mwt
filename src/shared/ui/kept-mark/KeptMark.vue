<script setup lang="ts">
import { Pin } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'

/** The «kept by hand» mark of a transaction or a reminder: sync will not merge this record away. */
const { t } = useI18n()
</script>

<template>
  <span class="flex shrink-0 items-center text-muted-foreground" :title="t('keep.mark')" data-kept-mark>
    <Pin class="size-3" aria-hidden="true" />
    <span class="sr-only">{{ t('keep.mark') }}</span>
  </span>
</template>
