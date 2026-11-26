<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { builtinThemes } from '@/shared/lib/theme/registry'
import type { Theme, ThemeMode } from '@/shared/lib/theme/types'
import { Button } from '@/shared/ui/button'

/**
 * Appearance picker: the built-in themes grouped light / dark, each with a
 * swatch drawn from the theme's own data. One radiogroup, roving tabindex:
 * arrows move the focus, Enter or Space selects.
 */
const props = defineProps<{ modes: ThemeMode | 'both'; selectedId: string }>()
const emit = defineEmits<{ select: [id: string] }>()

const { t } = useI18n()

const GROUP_ORDER: readonly ThemeMode[] = ['light', 'dark']
const SWATCH_TOKENS = ['--background', '--card', '--primary', '--money-income', '--money-expense'] as const

const groups = computed(() =>
  GROUP_ORDER.filter((mode) => props.modes === 'both' || props.modes === mode).map((mode) => ({
    mode,
    themes: builtinThemes.filter((theme) => theme.mode === mode),
  })),
)
const flat = computed(() => groups.value.flatMap((group) => group.themes))

const tabStopId = computed(() =>
  flat.value.some((theme) => theme.id === props.selectedId) ? props.selectedId : flat.value[0]?.id,
)

function swatchStyle(theme: Theme, token: (typeof SWATCH_TOKENS)[number]) {
  return { backgroundColor: theme.tokens[token] }
}

function move(event: KeyboardEvent, from: number, step: number) {
  const count = flat.value.length
  const group = (event.currentTarget as HTMLElement).closest('[role="radiogroup"]')
  const targetId = flat.value[(from + step + count) % count]?.id
  group?.querySelector<HTMLElement>(`[data-theme-id="${CSS.escape(targetId ?? '')}"]`)?.focus()
}

function onKeydown(event: KeyboardEvent, theme: Theme) {
  const index = flat.value.indexOf(theme)
  if (event.key === 'ArrowRight' || event.key === 'ArrowDown') move(event, index, 1)
  else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') move(event, index, -1)
  else if (event.key === 'Enter' || event.key === ' ') emit('select', theme.id)
  else return
  event.preventDefault()
}
</script>

<template>
  <div role="radiogroup" :aria-label="t('themePicker.label')" class="flex flex-col gap-4">
    <section v-for="group in groups" :key="group.mode" class="flex flex-col gap-2">
      <h3 class="text-sm font-medium text-muted-foreground">{{ t(`themePicker.group.${group.mode}`) }}</h3>
      <div class="grid grid-cols-2 gap-2">
        <!-- biome-ignore lint/a11y/useSemanticElements: a themed swatch card, not a bare radio input -->
        <Button
          v-for="theme in group.themes"
          :key="theme.id"
          variant="outline"
          size="block"
          role="radio"
          :data-theme-id="theme.id"
          :aria-checked="theme.id === selectedId"
          :tabindex="theme.id === tabStopId ? 0 : -1"
          class="flex-col items-stretch gap-2 border aria-checked:border-primary aria-checked:ring-2 aria-checked:ring-primary"
          @click="emit('select', theme.id)"
          @keydown="onKeydown($event, theme)"
        >
          <span class="flex overflow-hidden rounded-sm border" aria-hidden="true">
            <span
              v-for="token in SWATCH_TOKENS"
              :key="token"
              data-swatch
              class="h-5 flex-1"
              :style="swatchStyle(theme, token)"
            />
          </span>
          <span class="truncate">{{ theme.name }}</span>
        </Button>
      </div>
    </section>
  </div>
</template>
