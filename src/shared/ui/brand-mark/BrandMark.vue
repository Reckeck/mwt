<script setup lang="ts">
// The app icon's small build (crates/app/icons/src/icon-small.svg), copied to
// public/favicon.svg by `mise run icons`: one source for tab, tray and header.
import { cn } from '@/shared/lib/utils'

defineProps<{ class?: string }>()

const src = `${import.meta.env.BASE_URL}favicon.svg`
</script>

<template>
  <img :src="src" alt="" aria-hidden="true" draggable="false" :class="cn('aspect-square shrink-0 select-none', $props.class)" />
</template>
