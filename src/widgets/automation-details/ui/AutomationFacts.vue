<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { MoneyPhrase } from '@/entities/currency'

import { cardDo, cardIf, cardNext, cardWhen } from '../lib/card-view'

/** `When`, `If`, `Do`, `Ask `Yes``, `Next time` (Auto-1 · 3). */
const props = defineProps<{
  card: AutomationCard
  names: { store: (id: string) => string; category: (key: string) => string }
}>()

const { t } = useI18n()
const rows = computed(() => [
  { key: 'when', value: cardWhen(props.card, t) },
  { key: 'if', value: cardIf(props.card.trigger, props.names, t) },
  { key: 'do', value: cardDo(props.card, props.names, t) },
  { key: 'asks', value: t(props.card.asks ? 'automationCard.yes' : 'automationCard.no') },
  { key: 'next', value: cardNext(props.card.next_at, t) },
])
</script>

<template>
  <dl class="flex flex-col gap-4" data-automation-facts>
    <div v-for="row in rows" :key="row.key" class="flex justify-between gap-3 py-1.5 text-sm" :data-automation-fact="row.key">
      <dt class="shrink-0 text-muted-foreground">{{ t(`automationCard.row.${row.key}`) }}</dt>
      <dd class="min-w-0 text-end">
        <template v-if="typeof row.value === 'string'">{{ row.value }}</template>
        <template v-else>
          <MoneyPhrase :parts="row.value" />
        </template>
      </dd>
    </div>
  </dl>
</template>
