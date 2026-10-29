<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { useAssetClassAssign } from '@/features/asset-class-edit'
import { AssetClassSelect, assetClassName, suggestClass } from '@/entities/asset-class'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { assetKey } from '@/shared/lib/asset-ref'
import { useSettingsStore } from '@/shared/store/settings'
import { Button } from '@/shared/ui/button'
import { EntityLink } from '@/shared/ui/entity-link'

/**
 * Every held asset with its own basket. `By wallet` leaves it to the
 * wallet's basket; the hint (crypto → risky, cash by currency) is applied
 * only by a tap.
 */
const props = defineProps<{ view: AssetClassesView }>()
const { t } = useI18n()
const settings = useSettingsStore()
const assign = useAssetClassAssign()

const summary = useQuery({ queryKey: ['summary'], queryFn: () => call<PortfolioSummary>('get_summary') })
const budgetCurrency = computed(() => settings.vault?.budget_currency || settings.baseCurrency)

const rows = computed(() =>
  (summary.data.value?.assets ?? [])
    .filter((stat) => Number(stat.value_base) > 0)
    .map((stat) => {
      const key = assetKey(stat.asset)
      const linked = props.view.links.find((link) => link.asset_key === key)?.class_id ?? ''
      const hint = linked ? null : suggestClass(stat.asset, budgetCurrency.value, props.view.classes)
      return { key, stat, linked, hint: props.view.classes.find((cls) => cls.id === hint) ?? null }
    }),
)

function pick(key: string, classId: string, previous: string) {
  assign.mutate({ target: { kind: 'asset', asset_key: key }, classId: classId || null, previous: previous || null })
}
</script>

<template>
  <ul v-if="rows.length" class="grid gap-2 text-sm">
    <li v-for="row in rows" :key="row.key" class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 gap-y-1">
      <div class="flex min-w-0 items-center gap-2">
        <EntityLink :target="{ kind: 'asset', id: row.key }" :label="row.stat.asset.symbol" />
        <Money class="text-xs text-muted-foreground" :amount="row.stat.value_base" :currency="summary.data.value?.base_currency" :decimals="0" />
      </div>
      <AssetClassSelect
        class="w-48"
        :classes="props.view.classes"
        :empty-label="t('assetClass.byStore')"
        :model-value="row.linked"
        :disabled="assign.isPending.value"
        :aria-label="t('assetClass.assetLabel', { asset: row.stat.asset.symbol })"
        @update:model-value="(id: string) => pick(row.key, id, row.linked)"
      />
      <Button
        v-if="row.hint"
        variant="link"
        size="sm"
        class="col-span-2 h-auto justify-self-start p-0 text-xs"
        @click="pick(row.key, row.hint.id, '')"
      >
        {{ t('assetClass.suggest', { name: assetClassName(row.hint, t) }) }}
      </Button>
    </li>
  </ul>
  <p v-else class="text-sm text-muted-foreground">{{ t('assetClass.noAssets') }}</p>
</template>
