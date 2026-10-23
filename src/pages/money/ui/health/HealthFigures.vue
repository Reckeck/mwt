<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'

import { fundCanImprove, healthFigures, healthPhrase, type HealthTone } from '../../lib/health-view'

/**
 * The checkup (Money 3 · 16): one sentence about the main thing, then fund,
 * debts and savings rate. Cards on a desktop, rows of one list on a phone.
 */
const props = defineProps<{ report: HealthReport }>()
const emit = defineEmits<{ setupFund: [] }>()
const { t } = useI18n()

const TONE: Record<HealthTone, string> = {
  success: 'text-success',
  warning: 'text-warning',
  destructive: 'text-destructive',
  muted: '',
}

const phrase = computed(() => healthPhrase(props.report, t))
const figures = computed(() => healthFigures(props.report, t))
const fundSetup = computed(() => fundCanImprove(props.report.emergency))
</script>

<template>
  <div class="grid gap-3" data-tour="health-figures">
    <p class="rounded-xl border bg-card px-4 py-3.5 text-lg font-semibold text-balance shadow-sm max-md:text-sm" data-health-phrase>
      {{ phrase }}
    </p>
    <div class="grid grid-cols-3 gap-3 max-md:grid-cols-1 max-md:gap-0 max-md:rounded-xl max-md:border max-md:bg-card max-md:px-3">
      <section
        v-for="figure in figures"
        :key="figure.key"
        :class="[
          'grid content-start gap-1 rounded-xl border bg-card p-4 shadow-sm',
          'max-md:grid-cols-[1fr_auto] max-md:gap-x-3 max-md:gap-y-0.5 max-md:rounded-none max-md:border-0 max-md:border-b',
          'max-md:bg-transparent max-md:px-0 max-md:py-2.5 max-md:shadow-none max-md:last:border-b-0',
        ]"
        :data-health-figure="figure.key"
      >
        <span class="text-xs text-muted-foreground max-md:text-sm max-md:text-foreground">{{ t(`money.health.${figure.key}`) }}</span>
        <b class="text-xl tabular-nums max-md:row-span-2 max-md:self-center max-md:text-sm" :class="TONE[figure.tone]">
          {{ figure.value }}
        </b>
        <span class="text-xs text-muted-foreground">{{ figure.hint }}</span>
        <Button
          v-if="figure.key === 'fund' && fundSetup"
          variant="outline"
          size="sm"
          class="mt-1 w-fit max-md:col-span-2"
          data-health-fund-setup
          @click="emit('setupFund')"
        >
          {{ t('health.fund.edit') }}
        </Button>
      </section>
    </div>
  </div>
</template>
