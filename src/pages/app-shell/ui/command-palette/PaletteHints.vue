<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Kbd } from '@/shared/ui/kbd'
import { hotkeyLabel } from '@/shared/lib/hotkeys'

/**
 * The keys under the palette (Paths 1 · 7, 2 · 8): empty — choose, open,
 * glance, Alt 1…5; on a category — open in «Траты», glance only, record.
 * A sum to convert — copy, another currency, record an exchange (Currency-2 · 10).
 * A phone has no keys: a tap opens, a long press glances.
 */
const props = defineProps<{ category: boolean; converting: boolean; phone: boolean }>()
const { t } = useI18n()

const mod = computed(() => hotkeyLabel('saveAndNext').split(' ')[0])
const alt = computed(() => hotkeyLabel('section1').split(' ')[0])
const hints = computed(() => {
  if (props.converting)
    return [
      { key: 'Enter', label: t('palette.hint.copy') },
      { key: 'Tab', label: t('palette.hint.otherCurrency') },
      { key: `${mod.value} Enter`, label: t('palette.hint.recordExchange') },
      { key: 'Esc', label: t('palette.hint.close') },
    ]
  return props.category
    ? [
        { key: 'Enter', label: t('palette.hint.openSpending') },
        { key: 'Shift Enter', label: t('palette.hint.peekOnly') },
        { key: `${mod.value} Enter`, label: t('palette.hint.record') },
        { key: 'Esc', label: t('palette.hint.close') },
      ]
    : [
        { key: '↑↓', label: t('palette.hint.choose') },
        { key: 'Enter', label: t('palette.hint.open') },
        { key: 'Shift Enter', label: t('palette.hint.peek') },
        { key: `${alt.value} 1…5`, label: t('palette.hint.sections') },
      ]
})
</script>

<template>
  <p v-if="props.phone" class="border-t px-4 py-2.5 text-xs text-muted-foreground" data-palette-hints>
    {{ t(props.converting ? 'palette.hint.phoneConvert' : 'palette.hint.phone') }}
  </p>
  <div v-else class="flex flex-wrap items-center gap-x-3.5 gap-y-1 border-t px-3.5 py-2 text-xs text-muted-foreground" data-palette-hints>
    <span v-for="hint in hints" :key="hint.key" class="whitespace-nowrap"><Kbd>{{ hint.key }}</Kbd> {{ hint.label }}</span>
  </div>
</template>
