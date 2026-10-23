<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { useTxForm } from '../../model/use-tx-form'
import { QUICK_KINDS } from '../../transaction-form.constants'

/**
 * Оплата · Обмен · Перевод · Пополнение as one segmented row (OpForm-1 · 1); a
 * wallet without one of them just has fewer. A deal (OpForm-2 · 9) passes its
 * groups: each is a segmented block of its own, side by side.
 */
const { t } = useI18n()
const { form, offered, pickKind, props } = useTxForm()

const groups = computed(() =>
  (props.kindGroups ?? [QUICK_KINDS])
    .map((group) => group.filter((kind) => offered.value.includes(kind)))
    .filter((group) => group.length),
)
const shown = computed(() => groups.value.flat().length > 1)
</script>

<template>
  <div v-if="shown" class="flex flex-wrap items-center gap-2" data-tour="tx-kind-tabs">
    <ToggleGroup
      v-for="(group, index) in groups"
      :key="index"
      type="single"
      variant="segmented"
      :spacing="1"
      class="justify-start"
      :class="groups.length === 1 && 'w-full'"
      :model-value="form.kind"
      :aria-label="t('form.kind')"
      @update:model-value="pickKind"
    >
      <ToggleGroupItem v-for="kind in group" :key="kind" :value="kind" class="px-3" :data-kind-tab="kind">
        {{ t(`tx.kindTab.${kind}`) }}
      </ToggleGroupItem>
    </ToggleGroup>
  </div>
</template>
