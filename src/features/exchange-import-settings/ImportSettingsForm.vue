<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { X } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { CategorySelect } from '@/features/category-select'
import { AssetInput } from '@/entities/asset'
import { StoreNodeSelect } from '@/entities/store'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { Switch } from '@/shared/ui/switch'
import { call, parseError } from '@/shared/api/ipc'
import { toIsoTimestamp } from '@/shared/lib/datetime'
import { useFormValidation } from '@/shared/lib/form'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import { IMPORT_ACCOUNTS, IMPORT_KINDS } from './exchange-import-settings.constants'
import type { ImportRouteMode, ImportSettingsFormProps } from './exchange-import-settings.types'

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
const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
/** The wallet and its sub-accounts: where an account may book. */
const targets = computed(() => {
  const root = (tree.data.value ?? []).find((node) => node.store.id === props.storeId)
  return root ? flattenStoreTree([root]).filter((node) => !node.store.archived) : []
})

const flags = reactive({ trades: true, transfers: true, earn: true, auto_confirm: false, auto_reconcile: true })
const categories = reactive<Record<(typeof IMPORT_KINDS)[number], string>>({ trades: '', transfers: '', earn: '' })
const routes = reactive<Record<string, { mode: ImportRouteMode; storeId: string }>>(
  Object.fromEntries(IMPORT_ACCOUNTS.map((account) => [account, { mode: 'default', storeId: '' }])),
)
const coins = ref<string[]>([])
const since = ref('')
const coinPick = ref<AssetRef | null>(null)

watch(
  () => stored.data.value,
  (settings) => {
    if (!settings) return
    Object.assign(flags, {
      trades: settings.trades,
      transfers: settings.transfers,
      earn: settings.earn,
      auto_confirm: settings.auto_confirm,
      auto_reconcile: settings.auto_reconcile,
    })
    for (const kind of IMPORT_KINDS) categories[kind] = settings.categories[kind] ?? ''
    for (const account of IMPORT_ACCOUNTS) {
      const route = settings.accounts.find((entry) => entry.account === account)
      routes[account] = route
        ? { mode: route.store_id ? 'store' : 'skip', storeId: route.store_id ?? '' }
        : { mode: 'default', storeId: '' }
    }
    coins.value = [...settings.coins]
    since.value = settings.since?.slice(0, 10) ?? ''
  },
  { immediate: true },
)

// A coin picked in the search joins the list at once.
watch(coinPick, (asset) => {
  if (!asset) return
  const symbol = asset.symbol.toUpperCase()
  if (!coins.value.includes(symbol)) coins.value = [...coins.value, symbol]
  coinPick.value = null
})

/** A route «into a chosen account» without the account is the only way a route is wrong. */
const routeError = (account: (typeof IMPORT_ACCOUNTS)[number]) =>
  routes[account].mode === 'store' && !routes[account].storeId ? t('importSettings.pickWallet') : null

const form = useFormValidation(() => ({
  'route-FUND': routeError('FUND'),
  'route-UNIFIED': routeError('UNIFIED'),
  'route-EARN': routeError('EARN'),
  since: since.value && !toIsoTimestamp(since.value) ? t('validation.date') : null,
}))

const save = useMutation({
  mutationFn: () => {
    const settings: ImportSettings = {
      ...flags,
      coins: coins.value,
      since: since.value ? toIsoTimestamp(since.value) : null,
      categories: {
        trades: categories.trades || null,
        transfers: categories.transfers || null,
        earn: categories.earn || null,
      },
      accounts: IMPORT_ACCOUNTS.filter((account) => routes[account].mode !== 'default').map((account) => ({
        account,
        store_id: routes[account].mode === 'store' ? routes[account].storeId : null,
      })),
    }
    return call<ImportSettings>('set_import_settings', { input: { store_id: props.storeId, settings } })
  },
  onSuccess: (saved) => {
    queryClient.setQueryData(queryKey.value, saved)
    toast.success(t('importSettings.saved'))
  },
  onError: (err) => toast.error(t('importSettings.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <form
    id="import-settings"
    class="@container grid gap-6"
    data-tour="import-settings"
    @submit.prevent="form.submit(save.mutate, $event)"
  >
    <p class="text-sm text-muted-foreground">{{ t('importSettings.intro') }}</p>

    <section class="grid gap-3" data-tour="import-routes">
      <h3 class="text-sm font-semibold">{{ t('importSettings.where') }}</h3>
      <div v-for="account in IMPORT_ACCOUNTS" :key="account" class="grid items-start gap-2 @md:grid-cols-[11rem_13rem_1fr]">
        <Label :for="`import-route-${account}`" class="pt-2">{{ t(`exchangeBalances.account.${account}`) }}</Label>
        <Select :id="`import-route-${account}`" v-model="routes[account].mode">
          <option value="default">{{ t('importSettings.mode.default') }}</option>
          <option value="store">{{ t('importSettings.mode.store') }}</option>
          <option value="skip">{{ t('importSettings.mode.skip') }}</option>
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
      <div v-for="kind in IMPORT_KINDS" :key="kind" class="grid items-center gap-2 @md:grid-cols-[12rem_1fr]">
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
            <button
              type="button"
              class="rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              :aria-label="t('importSettings.removeCoin', { coin })"
              @click="coins = coins.filter((entry) => entry !== coin)"
            >
              <X class="size-3" />
            </button>
          </Badge>
        </div>
        <AssetInput v-model="coinPick" label="" :placeholder="t('importSettings.addCoin')" />
        <p class="text-xs text-muted-foreground">{{ t('importSettings.coinsHint') }}</p>
      </div>
      <FormField for="import-since" :label="t('importSettings.since')" :error="form.error('since')" :hint="t('importSettings.sinceHint')" class="@md:max-w-64">
        <Input id="import-since" v-model="since" type="date" />
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

    <div>
      <Button type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
