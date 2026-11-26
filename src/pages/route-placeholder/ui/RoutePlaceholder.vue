<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { EmptyState } from '@/shared/ui/empty-state'

/**
 * The body of every route whose screen is being rebuilt (spec D14). The task that builds
 * a screen replaces this component in the route; when none is left, the page is deleted.
 */
const { t } = useI18n()
</script>

<template>
  <EmptyState data-route-placeholder :title="t('routePlaceholder.title')" :description="t('routePlaceholder.description')" />
</template>
