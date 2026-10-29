<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { diffTone, sumDecimals } from '@/entities/shared-event'
import { StatCard } from '@/shared/ui/stat-card'
import { TapHint } from '@/shared/ui/tap-hint'

/**
 * `Planned / Spent / Difference` and the shares in one line. Overspending is red
 * (+600), staying under the plan is green; only what was bought counts.
 */
const props = defineProps<{ view: SharedEventView }>()
const { t } = useI18n()

const currency = computed(() => props.view.event.currency)
const decimals = computed(() => sumDecimals(props.view))
/** Over the plan on what was bought (the backend sums the bought planned items): the beer cost 600 more. */
const diff = computed(() => props.view.totals.diff)
const tone = computed(() => diffTone(diff.value))
const TONE_CLASS = { over: 'text-destructive', under: 'text-success', even: '' } as const
</script>

<template>
  <div class="grid gap-3" data-tour="shared-totals">
    <div class="grid grid-cols-3 gap-2 md:gap-3 [&>*]:min-w-0">
      <StatCard :label="t('sharedEvents.totals.planned')">
        <Money :amount="props.view.totals.planned" :currency="currency" :decimals="decimals" :convert="false" />
      </StatCard>
      <StatCard :label="t('sharedEvents.totals.spent')">
        <Money :amount="props.view.totals.spent" :currency="currency" :decimals="decimals" :convert="false" />
      </StatCard>
      <StatCard :label="t('sharedEvents.totals.diff')">
        <template #label>
          <TapHint :label="t('sharedEvents.totals.diff')" :hint="t('sharedEvents.totals.diffHint')" />
        </template>
        <Money
          :amount="diff"
          :currency="currency"
          :decimals="decimals"
          :convert="false"
          signed
          :class="TONE_CLASS[tone]"
        />
      </StatCard>
    </div>
    <p class="flex flex-wrap items-baseline gap-x-2 gap-y-1 rounded-lg bg-muted px-3 py-2 text-sm text-muted-foreground" data-tour="shared-shares">
      <span>{{ t('sharedEvents.totals.shares') }}</span>
      <template v-for="(person, index) in props.view.people" :key="person.id">
        <span v-if="index > 0" aria-hidden="true">·</span>
        <span>
          {{ person.name || t('sharedEvents.me') }}
          <Money :amount="person.share" :currency="currency" :decimals="decimals" :convert="false" bare class="font-semibold text-foreground" />
        </span>
      </template>
    </p>
  </div>
</template>
