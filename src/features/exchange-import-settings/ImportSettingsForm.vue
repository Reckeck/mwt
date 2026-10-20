<script setup lang="ts">
// check-ui-ignore icon-button-size: the × inside a coin chip is 16px like the chip text; touch-hit-lg gives it the finger area
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Eye, X } from 'lucide-vue-next'

import { CategorySelect } from '@/features/category-select'
import { AssetInput } from '@/entities/asset'
import { StoreNodeSelect } from '@/entities/store'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { Switch } from '@/shared/ui/switch'
import { call } from '@/shared/api/ipc'
import { toIsoTimestamp } from '@/shared/lib/datetime'
import { useFormValidation } from '@/shared/lib/form'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { notify } from '@/shared/lib/notify'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import { IMPORT_ACCOUNTS, IMPORT_KINDS } from './exchange-import-settings.constants'
import type { ImportSettingsFormProps } from './exchange-import-settings.types'
import { useImportSettingsDraft } from './model/use-import-settings-draft'
import { useStoreTree } from '@/shared/api/queries'

/**
 * What an exchange import brings in and where it books it: set up right after
 * the key is saved. Each Bybit account goes to its bound sub-account, to a
 * wallet of the user's choice, or is not imported; each kind of operation can
 * be switched off and given its category; coins and an earliest date narrow it.
 */
const props = defineProps<ImportSettingsFormProps>()
const { t } = useI18n()
const queryClient = useQueryClient()

const queryKey = computed(() => ['import-settings', props.storeId])
const stored = useQuery({
  queryKey,
  queryFn: () => call<ImportSettings>('get_import_settings', { storeId: props.storeId }),
})
const tree = useStoreTree()
/** The wallet and its sub-accounts: where an account may book. */
const targets = computed(() => {
  const root = (tree.data.value ?? []).find((node) => node.store.id === props.storeId)
  return root ? flattenStoreTree([root]).filter((node) => !node.store.archived) : []
})

const { flags, categories, routes, coins, since, coinPick, currentSettings } = useImportSettingsDraft(
  () => stored.data.value,
)

/** A route «into a chosen account» without the account is the only way a route is wrong. */
const routeError = (account: (typeof IMPORT_ACCOUNTS)[number]) =>
  routes[account].mode === 'store' && !routes[account].storeId ? t('importSettings.pickWallet') : null

const form = useFormValidation(() => ({
  'route-FUND': routeError('FUND'),
  'route-UNIFIED': routeError('UNIFIED'),
  'route-EARN': routeError('EARN'),
  since: since.value && !toIsoTimestamp(since.value) ? t('validation.date') : null,
}))

/** What a sync would bring with these settings, before saving them: nothing is staged. */
const preview = useMutation({
  mutationFn: () =>
    call<ImportPreviewOutput>('preview_bybit_import', {
      input: { store_id: props.storeId, settings: currentSettings() },
    }),
  onSuccess: (result) => {
    if (result.notice) notify.warning(`import.notice.${result.notice}`)
  },
  meta: { silent: 'the preview table below is the result', error: 'importSettings.previewFailed' },
})
/** Rows the preview lists; the counts above cover them all. */
const PREVIEW_ROWS = 20

const save = useMutation({
  mutationFn: () => {
    const settings = currentSettings()
    return call<ImportSettings>('set_import_settings', { input: { store_id: props.storeId, settings } })
  },
  onSuccess: (saved) => {
    queryClient.setQueryData(queryKey.value, saved)
  },
  meta: { error: 'importSettings.saveFailed', success: 'importSettings.saved' },
})
</script>

<template>
  <form
    id="import-settings"
    class="@container grid gap-6"
    @submit.prevent="form.submit(save.mutate, $event)"
  >
    <p class="text-sm text-muted-foreground">{{ t('importSettings.intro') }}</p>

    <section class="grid gap-3" data-tour="import-routes">
      <h3 class="text-sm font-semibold">{{ t('importSettings.where') }}</h3>
      <div v-for="account in IMPORT_ACCOUNTS" :key="account" class="grid items-start gap-2 @mid:grid-cols-[11rem_13rem_1fr]">
        <Label :for="`import-route-${account}`" class="pt-2">{{ t(`exchangeBalances.account.${account}`) }}</Label>
        <Select :id="`import-route-${account}`" v-model="routes[account].mode">
          <SelectItem value="default">{{ t('importSettings.mode.default') }}</SelectItem>
          <SelectItem value="store">{{ t('importSettings.mode.store') }}</SelectItem>
          <SelectItem value="skip">{{ t('importSettings.mode.skip') }}</SelectItem>
        </Select>
        <FormField v-if="routes[account].mode === 'store'" :error="form.error(`route-${account}`)">
          <StoreNodeSelect
            v-model="routes[account].storeId"
            :nodes="targets"
            flat
            :placeholder="t('importSettings.pickWallet')"
            :invalid="Boolean(form.error(`route-${account}`))"
            :aria-label="t('importSettings.target', { account: t(`exchangeBalances.account.${account}`) })"
          />
        </FormField>
        <p v-else class="pt-2 text-xs text-muted-foreground">{{ t(`importSettings.modeHint.${routes[account].mode}`) }}</p>
      </div>
    </section>

    <section class="grid gap-3" data-tour="import-kinds">
      <h3 class="text-sm font-semibold">{{ t('importSettings.what') }}</h3>
      <div v-for="kind in IMPORT_KINDS" :key="kind" class="grid items-center gap-2 @mid:grid-cols-[12rem_1fr]">
        <div class="flex items-center gap-2">
          <Switch :id="`import-kind-${kind}`" v-model="flags[kind]" />
          <Label :for="`import-kind-${kind}`">{{ t(`importSettings.kind.${kind}`) }}</Label>
        </div>
        <CategorySelect
          v-if="flags[kind]"
          v-model="categories[kind]"
          :groups="kind === 'earn' ? ['income', 'service'] : ['service']"
          :placeholder="t(`importSettings.defaultCategory.${kind}`)"
          :aria-label="t('importSettings.categoryFor', { kind: t(`importSettings.kind.${kind}`) })"
          no-create
        />
      </div>
      <p class="text-xs text-muted-foreground">{{ t('importSettings.markHint') }}</p>
    </section>

    <section class="grid gap-4">
      <h3 class="text-sm font-semibold">{{ t('importSettings.narrow') }}</h3>
      <div class="grid gap-1.5">
        <Label>{{ t('importSettings.coins') }}</Label>
        <div v-if="coins.length" class="flex flex-wrap gap-1.5">
          <Badge v-for="coin in coins" :key="coin" variant="secondary" class="gap-1">
            {{ coin }}
            <Button
              variant="ghost"
              size="icon-xs"
              class="size-4 rounded-sm"
              :aria-label="t('importSettings.removeCoin', { coin })"
              @click="coins = coins.filter((entry) => entry !== coin)"
            >
              <X class="size-3" />
            </Button>
          </Badge>
        </div>
        <AssetInput v-model="coinPick" label="" :placeholder="t('importSettings.addCoin')" />
        <p class="text-xs text-muted-foreground">{{ t('importSettings.coinsHint') }}</p>
      </div>
      <FormField for="import-since" :label="t('importSettings.since')" :error="form.error('since')" :hint="t('importSettings.sinceHint')" class="@mid:max-w-64">
        <DateTimePicker id="import-since" v-model="since" />
      </FormField>
    </section>

    <section class="grid gap-3">
      <h3 class="text-sm font-semibold">{{ t('importSettings.how') }}</h3>
      <div class="flex items-start gap-2">
        <Switch id="import-auto-confirm" v-model="flags.auto_confirm" />
        <div class="grid gap-0.5">
          <Label for="import-auto-confirm">{{ t('importSettings.autoConfirm') }}</Label>
          <p class="text-xs text-muted-foreground">{{ t('importSettings.autoConfirmHint') }}</p>
        </div>
      </div>
      <div class="flex items-start gap-2">
        <Switch id="import-auto-reconcile" v-model="flags.auto_reconcile" />
        <div class="grid gap-0.5">
          <Label for="import-auto-reconcile">{{ t('importSettings.autoReconcile') }}</Label>
          <p class="text-xs text-muted-foreground">{{ t('importSettings.autoReconcileHint') }}</p>
        </div>
      </div>
    </section>

    <section class="grid gap-2" data-tour="import-preview">
      <div class="flex flex-wrap items-center gap-2">
        <Button type="button" variant="outline" :disabled="preview.isPending.value" @click="form.submit(preview.mutate, $event)">
          <Eye class="size-4" />
          {{ t('importSettings.preview') }}
        </Button>
        <p class="text-xs text-muted-foreground">{{ t('importSettings.previewHint') }}</p>
      </div>
      <template v-if="preview.data.value?.preview">
        <p class="text-sm">
          {{
            t('importSettings.previewCounts', {
              fetched: preview.data.value.preview.fetched,
              new: preview.data.value.preview.new,
              known: preview.data.value.preview.known,
              filtered: preview.data.value.preview.filtered,
            })
          }}
        </p>
        <ul v-if="preview.data.value.preview.items.length" class="grid divide-y divide-border rounded-md border border-border text-sm">
          <li
            v-for="imp in preview.data.value.preview.items.slice(0, PREVIEW_ROWS)"
            :key="imp.exec_id"
            class="flex flex-wrap items-center gap-2 px-3 py-1.5"
          >
            <span class="font-mono text-xs text-muted-foreground">{{ imp.occurred_at.slice(0, 10) }}</span>
            <span>{{ imp.kind === 'fill' ? imp.symbol : imp.asset_bought }}</span>
            <Badge variant="outline">{{ t(`pending.kind.${imp.kind}`) }}</Badge>
            <span class="ml-auto tabular-nums">{{ imp.kind !== 'fill' || imp.side === 'buy' ? imp.qty_bought : imp.qty_sold }}</span>
          </li>
        </ul>
      </template>
    </section>

    <div>
      <Button type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
