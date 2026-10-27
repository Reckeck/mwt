<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { CircleAlert } from 'lucide-vue-next'

import { parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { Skeleton } from '@/shared/ui/skeleton'

import type { SectionStateProps } from './section-state.types'

/**
 * Loading, failed, empty or the content — one of them, never "nothing here"
 * while the data is still on its way.
 *
 * The skeleton takes its height at once (nothing below jumps when it
 * appears) but fades in only after a moment, so a fast answer does not flash
 * a placeholder. The empty block takes its action button from `#empty`; a page whose
 * content has its own height gives the placeholder in `#loading`.
 */
const props = withDefaults(defineProps<SectionStateProps>(), {
  loading: false,
  error: undefined,
  empty: false,
  emptyTitle: undefined,
  emptyDescription: undefined,
  emptyIcon: undefined,
  skeleton: 'list',
  rows: 3,
  compact: false,
})
const emit = defineEmits<{ retry: [] }>()

const { t } = useI18n()

const SKELETON_DELAY_MS = 200
const showSkeleton = ref(false)
let timer: ReturnType<typeof setTimeout> | undefined
watch(
  () => props.loading,
  (loading) => {
    clearTimeout(timer)
    if (!loading) {
      showSkeleton.value = false
      return
    }
    timer = setTimeout(() => {
      showSkeleton.value = true
    }, SKELETON_DELAY_MS)
  },
  { immediate: true },
)
onBeforeUnmount(() => clearTimeout(timer))

function errorText(err: unknown): string {
  return codeErrorMessage(parseError(err), t)
}
</script>

<template>
  <div
    v-if="props.loading"
    :aria-busy="true"
    :aria-label="t('ui.loading')"
    class="transition-opacity duration-200"
    :class="showSkeleton ? 'opacity-100' : 'opacity-0'"
  >
    <!-- `#loading`: a placeholder as tall as the content that will replace it, where the stock shapes are not. -->
    <slot name="loading">
      <div v-if="props.skeleton === 'chart'" class="flex flex-col gap-2">
        <Skeleton class="h-4 w-32" />
        <Skeleton class="h-48 w-full" />
      </div>
      <!-- A report: the facts in words, a chart, the first rows of its table. -->
      <div v-else-if="props.skeleton === 'report'" class="flex flex-col gap-4">
        <Skeleton class="h-20 w-full" />
        <Skeleton class="h-72 w-full" />
        <Skeleton v-for="row in props.rows" :key="row" class="h-10 w-full" />
      </div>
      <div v-else-if="props.skeleton === 'figure'" class="flex flex-col gap-2">
        <Skeleton class="h-3 w-24" />
        <Skeleton class="h-8 w-40" />
      </div>
      <div v-else class="flex flex-col gap-2">
        <Skeleton v-if="props.skeleton === 'table'" class="h-8 w-full" />
        <Skeleton v-for="row in props.rows" :key="row" class="h-10 w-full" />
      </div>
    </slot>
  </div>

  <Alert v-else-if="props.error" variant="destructive" role="alert">
    <CircleAlert />
    <AlertTitle>{{ t('ui.loadError') }}</AlertTitle>
    <AlertDescription class="flex flex-col items-start gap-2">
      <span>{{ errorText(props.error) }}</span>
      <Button variant="outline" size="sm" @click="emit('retry')">{{ t('actions.retry') }}</Button>
    </AlertDescription>
  </Alert>

  <EmptyState
    v-else-if="props.empty"
    :title="props.emptyTitle"
    :description="props.emptyDescription"
    :icon="props.emptyIcon"
    :compact="props.compact"
  >
    <slot name="empty" />
  </EmptyState>

  <slot v-else />
</template>
