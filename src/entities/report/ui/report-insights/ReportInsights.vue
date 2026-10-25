<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Info, TrendingDown, TrendingUp } from 'lucide-vue-next'

import { categoryKeyLabel } from '@/shared/lib/categories'
import { activeLocale } from '@/shared/lib/locale'
import { partsText } from '@/shared/lib/money-parts'
import { cn } from '@/shared/lib/utils'

import { type InsightTone, insightText, insightTone } from '../../lib/insight-text'
import type { ReportInsightsProps } from './report-insights.types'
import { useCategories } from '@/shared/api/queries'

/** What a report means, in sentences; the icon repeats the colour's direction. */
const props = withDefaults(defineProps<ReportInsightsProps>(), { compact: false })

const { t } = useI18n()
const categories = useCategories()

const ICONS = { good: TrendingUp, bad: TrendingDown, neutral: Info }
const TONE_CLASS: Record<InsightTone, string> = {
  good: 'text-success',
  bad: 'text-destructive',
  neutral: 'text-muted-foreground',
}

const lines = computed(() =>
  (props.compact ? props.insights.slice(0, 1) : props.insights).map((fact) => {
    const tone = insightTone(fact)
    return {
      tone,
      icon: ICONS[tone],
      text: insightText(fact, {
        t,
        currency: props.currency,
        locale: activeLocale.value,
        category: (key) => (key ? categoryKeyLabel(key, categories.data.value, t) : t('report.categories.none')),
      }),
    }
  }),
)
</script>

<template>
  <ul v-if="lines.length" :class="cn('grid gap-1.5', 'text-sm')">
    <li v-for="(line, index) in lines" :key="index" class="flex items-start gap-2">
      <component :is="line.icon" :class="cn('mt-0.5 size-4 shrink-0', TONE_CLASS[line.tone])" aria-hidden="true" />
      <span :class="cn(index === 0 && !props.compact && 'font-medium')">
        <template v-for="(part, at) in line.text" :key="at">
          <!-- A sum is drawn by the consumer (a widget may use <Money>; an entity may not import it). -->
          <slot v-if="'amount' in part" name="sum" :sum="part">{{ partsText([part]) }}</slot>
          <template v-else>{{ part.text }}</template>
        </template>
      </span>
    </li>
  </ul>
</template>
