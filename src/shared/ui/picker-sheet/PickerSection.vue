<script setup lang="ts">
/**
 * A group of Picker rows (board 1097003): an 11/14 caption on the left, an optional total on
 * the right (`≈ 618 000 ₽` for crypto), then its rows.
 */
const props = defineProps<{ label: string; total?: string }>()
</script>

<template>
  <section class="grid">
    <h3 class="flex min-h-6 items-center justify-between gap-2 px-2 text-2xs font-medium text-muted-foreground">
      <span class="truncate">{{ props.label }}</span>
      <!-- money: plain — the caller passes the group's total preformatted -->
      <span v-if="props.total" class="shrink-0 font-normal tabular-nums">{{ props.total }}</span>
    </h3>
    <slot />
  </section>
</template>
