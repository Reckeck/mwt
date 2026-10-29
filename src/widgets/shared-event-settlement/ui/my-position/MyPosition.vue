<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { absolute, sign } from '@/entities/shared-event'
import { Card } from '@/shared/ui/card'
import { StatCard } from '@/shared/ui/stat-card'

/**
 * «Моя позиция»: what I paid, my share and what that leaves — owed to me, or to pay the
 * organizer. Returned, written off and out-of-pocket-beyond-share show only when they are not zero.
 */
const props = defineProps<{ mine: MyPosition; currency: string; decimals: number; organizerName: string }>()
const { t } = useI18n()

/** The headline: «Мне должны» in green, or «К оплате Ане» in red when I owe the organizer. */
const headline = computed(() => {
  const owe = sign(props.mine.i_owe) > 0
  const owed = sign(props.mine.owed_to_me) > 0
  if (owe && !owed) return { label: t('sharedEvents.mine.toPay', { name: props.organizerName }), amount: props.mine.i_owe, tone: 'negative' as const }
  return { label: t('sharedEvents.mine.owedToMe'), amount: props.mine.owed_to_me, tone: owed ? ('positive' as const) : ('neutral' as const) }
})

/** «Вернули» is negative when I paid the organizer: then it reads «Я отдал». «Сверх доли» only when it adds to the headline. */
const details = computed(() => {
  const returned = props.mine.returned
  const over = props.mine.over_share
  return [
    {
      id: 'returned',
      label: sign(returned) < 0 ? t('sharedEvents.mine.gaveBack') : t('sharedEvents.mine.returned'),
      amount: absolute(returned),
    },
    { id: 'written', label: t('sharedEvents.mine.writtenOff'), amount: props.mine.written_off },
    {
      id: 'over',
      label: t('sharedEvents.mine.overShare'),
      amount: sign(over) > 0 && over !== headline.value.amount ? over : '0',
    },
  ].filter((row) => sign(row.amount) !== 0)
})
</script>

<template>
  <div class="grid grid-cols-2 gap-3 md:grid-cols-4 [&>*]:min-w-0" data-tour="shared-my-position">
    <StatCard :label="t('sharedEvents.mine.paid')">
      <Money :amount="props.mine.paid" :currency="props.currency" :decimals="props.decimals" :convert="false" />
    </StatCard>
    <StatCard :label="t('sharedEvents.mine.share')">
      <Money :amount="props.mine.share" :currency="props.currency" :decimals="props.decimals" :convert="false" />
    </StatCard>
    <StatCard :label="headline.label" :tone="headline.tone">
      <Money :amount="headline.amount" :currency="props.currency" :decimals="props.decimals" :convert="false" />
    </StatCard>
    <Card v-if="details.length" size="sm">
      <dl class="grid gap-1 text-xs">
        <div v-for="row in details" :key="row.id" class="flex items-baseline justify-between gap-2">
          <dt class="truncate text-muted-foreground">{{ row.label }}</dt>
          <dd class="font-semibold tabular-nums">
            <Money :amount="row.amount" :currency="props.currency" :decimals="props.decimals" :convert="false" />
          </dd>
        </div>
      </dl>
    </Card>
  </div>
</template>
