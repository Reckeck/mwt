<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { assetClassName } from '@/entities/asset-class'
import { Money } from '@/entities/currency'
import { useAssetClasses } from '@/shared/api/queries'

/** The net worth's parts, with debts by loan kind and financial assets by basket. */
const props = defineProps<{ worth: NetWorth }>()
const { t } = useI18n()

const rows = computed(() =>
  [
    { key: 'financial', value: props.worth.financial, sign: 1 },
    { key: 'non_financial', value: props.worth.non_financial, sign: 1 },
    { key: 'receivables', value: props.worth.receivables, sign: 1 },
    { key: 'liabilities', value: props.worth.liabilities, sign: -1 },
    { key: 'payables', value: props.worth.payables, sign: -1 },
  ].filter((row) => Number(row.value) !== 0),
)

/** Debts by loan kind, under `Loans`; one kind would only repeat the total. */
const loanKinds = computed(() => {
  const shares = props.worth.liabilities_by_kind ?? []
  return shares.length > 1 ? shares : []
})

const classes = useAssetClasses()

/** Financial assets by basket, under `Financial`; shown once there is more than one, or a fund to set apart. */
const classShares = computed(() => {
  const data = props.worth
  const fund = Number(data.financial_fund)
  if (data.financial_by_class.length < 2 && fund <= 0) return []
  const known = classes.data.value?.classes ?? []
  const shares = data.financial_by_class.map((share) => {
    const cls = known.find((entry) => entry.id === share.class_id)
    return {
      key: share.class_id ?? '',
      label: cls ? assetClassName(cls, t) : t('assetClass.none'),
      amount: share.amount,
    }
  })
  return fund > 0 ? [...shares, { key: '#fund', label: t('assetClass.fundRow'), amount: data.financial_fund }] : shares
})
</script>

<template>
  <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs tabular-nums">
    <template v-for="row in rows" :key="row.key">
      <dt class="text-muted-foreground">{{ t(`report.networth.${row.key}`) }}</dt>
      <dd class="text-right" :class="row.sign < 0 && 'text-destructive'">
        <Money :amount="row.sign < 0 ? -Number(row.value) : row.value" :currency="worth.base_currency" :decimals="0" />
      </dd>
      <template v-if="row.key === 'financial'">
        <template v-for="share in classShares" :key="share.key">
          <dt class="truncate pl-3 text-muted-foreground">{{ share.label }}</dt>
          <dd class="text-right text-muted-foreground">
            <Money :amount="share.amount" :currency="worth.base_currency" :decimals="0" />
          </dd>
        </template>
      </template>
      <template v-if="row.key === 'liabilities'">
        <template v-for="share in loanKinds" :key="share.kind">
          <dt class="pl-3 text-muted-foreground">{{ t(`liabilityKind.${share.kind}`) }}</dt>
          <dd class="text-right text-muted-foreground">
            <Money :amount="-Number(share.amount)" :currency="worth.base_currency" :decimals="0" />
          </dd>
        </template>
      </template>
    </template>
  </dl>
</template>
