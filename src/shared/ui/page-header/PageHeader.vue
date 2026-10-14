<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'

import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/shared/ui/breadcrumb'

import type { PageHeaderProps } from './page-header.types'

/**
 * The top of a page: where it sits (breadcrumbs), what it is (title, badges in
 * the default slot) and what it does (`#actions`, the main action last).
 */
const props = withDefaults(defineProps<PageHeaderProps>(), {
  title: undefined,
  description: undefined,
  crumbs: () => [],
})

const { t } = useI18n()
</script>

<template>
  <header class="flex flex-col gap-2">
    <Breadcrumb v-if="props.crumbs.length" :aria-label="t('ui.breadcrumb')">
      <BreadcrumbList>
        <template v-for="(crumb, index) in props.crumbs" :key="`${index}-${crumb.label}`">
          <BreadcrumbItem class="min-w-0">
            <BreadcrumbLink v-if="crumb.to && index < props.crumbs.length - 1" as-child>
              <RouterLink :to="crumb.to" class="truncate">{{ crumb.label }}</RouterLink>
            </BreadcrumbLink>
            <BreadcrumbPage v-else class="truncate">{{ crumb.label }}</BreadcrumbPage>
          </BreadcrumbItem>
          <BreadcrumbSeparator v-if="index < props.crumbs.length - 1" />
        </template>
      </BreadcrumbList>
    </Breadcrumb>

    <div
      v-if="props.title || $slots.default || $slots.actions"
      class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2"
    >
      <div class="flex min-w-0 flex-1 flex-col gap-0.5">
        <div class="flex min-w-0 flex-wrap items-center gap-2">
          <h2 v-if="props.title" class="heading-page truncate">{{ props.title }}</h2>
          <slot />
        </div>
        <p v-if="props.description" class="text-sm text-muted-foreground">{{ props.description }}</p>
      </div>
      <div v-if="$slots.actions" class="flex shrink-0 flex-wrap items-center gap-2">
        <slot name="actions" />
      </div>
    </div>
  </header>
</template>
