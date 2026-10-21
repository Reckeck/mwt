<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { assetClassName } from '@/entities/asset-class'
import { Money } from '@/entities/currency'
import { formatPct } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'

/** Each basket: what it holds, its share now and after, the target and what to add. */
const props = defineProps<{ advice: TopupAdvice }>()
const { t } = useI18n()

const pct = (value: string | null) => formatPct(value, { decimals: 1 })
</script>

<template>
  <ul class="grid gap-2 text-sm">
    <li v-for="row in props.advice.rows" :key="row.class.id" class="grid gap-0.5">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <span class="flex items-center gap-2 font-medium">
          {{ assetClassName(row.class, t) }}
          <Badge v-if="row.capped" variant="outline">{{ t('assetClass.advisor.capped') }}</Badge>
        </span>
        <span class="font-medium text-success tabular-nums">
          + <Money :amount="row.add" :currency="props.advice.currency" :decimals="0" />
        </span>
      </div>
      <p class="text-xs text-muted-foreground tabular-nums">
        <Money :amount="row.value" :currency="props.advice.currency" :decimals="0" />
        · {{ t('assetClass.advisor.share', { before: pct(row.pct_before), after: pct(row.pct_after), target: pct(row.class.target_pct) }) }}
      </p>
    </li>
    <li v-if="Number(props.advice.unclassified) > 0" class="flex flex-wrap justify-between gap-2 text-xs text-muted-foreground">
      <span>{{ t('assetClass.advisor.unclassified') }}</span>
      <Money :amount="props.advice.unclassified" :currency="props.advice.currency" :decimals="0" />
    </li>
    <li v-if="Number(props.advice.emergency) > 0" class="flex flex-wrap justify-between gap-2 text-xs text-muted-foreground">
      <span>{{ t('assetClass.advisor.emergency') }}</span>
      <Money :amount="props.advice.emergency" :currency="props.advice.currency" :decimals="0" />
    </li>
  </ul>
</template>
