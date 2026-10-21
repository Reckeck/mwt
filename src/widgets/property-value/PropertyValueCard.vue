<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money, PnlText } from '@/entities/currency'
import { usePropertyValues } from '@/features/asset-revalue'
import { RevalueRuleDialog } from '@/features/property-revalue'
import { formatDate, formatPct, signClass } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'

/** What a property is worth now, how far it moved since the purchase, and the rule that moves it on. */
const props = defineProps<{ store: Store }>()
const { t } = useI18n()

const values = usePropertyValues(() => props.store.id)
const rows = computed(() => values.data.value ?? [])
const rule = computed(() => props.store.property?.revalue ?? null)
const currency = computed(() => rows.value[0]?.currency ?? props.store.property?.currency ?? 'usd')
const ruleOpen = ref(false)

const ruleText = computed(() => {
  const current = rule.value
  if (!current) return t('propertyValue.ruleOff')
  const pace =
    current.pct_per_year === null
      ? t('propertyValue.ruleInflation')
      : t('propertyValue.rulePct', { pct: formatPct(current.pct_per_year, { signed: true }) })
  const floor = current.floor
  if (!floor) return pace
  const limit =
    floor.kind === 'amount'
      ? `${floor.amount} ${currency.value.toUpperCase()}`
      : t('propertyValue.floorPct', { pct: formatPct(floor.pct) })
  return t('propertyValue.ruleWithFloor', { rule: pace, floor: limit })
})
</script>

<template>
  <Card data-tour="property-value">
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="min-w-0">
          <p class="truncate font-medium">{{ t('propertyValue.title') }}</p>
          <p class="text-xs text-muted-foreground">{{ ruleText }}</p>
        </div>
        <Button variant="outline" size="sm" @click="ruleOpen = true">{{ t('propertyValue.setRule') }}</Button>
      </div>
    </template>
    <p v-if="!rule" class="mb-3 text-sm text-muted-foreground">{{ t('propertyValue.ruleHint') }}</p>
    <p v-if="values.isSuccess.value && !rows.length" class="text-sm text-muted-foreground">
      {{ t('propertyValue.empty') }}
    </p>
    <dl v-for="row in rows" :key="row.asset_key" class="grid grid-cols-2 gap-x-4 gap-y-1.5 text-sm @mid:grid-cols-4">
      <div class="grid content-start gap-0.5">
        <dt class="text-xs text-muted-foreground">{{ t('propertyValue.now') }}</dt>
        <dd class="font-medium tabular-nums">
          <Money v-if="row.unit_price" :amount="row.unit_price" :currency="row.currency" :decimals="0" />
          <span v-else>—</span>
        </dd>
      </div>
      <div class="grid content-start gap-0.5">
        <dt class="text-xs text-muted-foreground">{{ t('propertyValue.purchase') }}</dt>
        <dd class="font-medium tabular-nums">
          <Money
            v-if="row.purchase"
            :amount="row.purchase.unit_price"
            :currency="row.purchase.currency"
            :decimals="0"
          />
          <span v-else>—</span>
        </dd>
        <dd v-if="row.purchase" class="text-xs text-muted-foreground">{{ formatDate(row.purchase.date) }}</dd>
      </div>
      <div class="grid content-start gap-0.5">
        <dt class="text-xs text-muted-foreground">{{ t('propertyValue.change') }}</dt>
        <dd class="font-medium tabular-nums">
          <PnlText v-if="row.change" :amount="row.change" :currency="row.currency" :decimals="0" :pct="row.change_pct" />
          <span v-else>—</span>
        </dd>
      </div>
      <div class="grid content-start gap-0.5">
        <dt class="text-xs text-muted-foreground">{{ t('propertyValue.real') }}</dt>
        <dd class="font-medium tabular-nums">
          <span :class="signClass(row.real_change_pct)">{{ formatPct(row.real_change_pct, { signed: true }) }}</span>
        </dd>
      </div>
    </dl>
    <RevalueRuleDialog v-model:open="ruleOpen" :store-id="props.store.id" :rule="rule" :currency="currency" />
  </Card>
</template>
