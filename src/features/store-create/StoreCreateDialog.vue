<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { ArrowLeft, Plus } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { AssetInput } from '@/entities/asset'
import { CurrencySelect } from '@/entities/currency'
import {
  LiabilityTermsFields,
  emptyLiabilityTerms,
  kindInfo,
  parsePairSymbol,
  useWalletCatalog,
} from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { call, parseError } from '@/shared/api/ipc'
import { fiatRef, tickerRef } from '@/shared/lib/asset-ref'
import { getStoreIcon, getSubTypeIcon } from '@/shared/lib/store-icons'
import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'
import { cn } from '@/shared/lib/utils'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

import { DEFAULT_STORE_COLOR, KIND_COLORS } from './store-create.constants'
import type { StoreCreateDialogProps } from './store-create.types'

const props = withDefaults(defineProps<StoreCreateDialogProps>(), { parent: null, parentPath: () => [], kind: null })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ created: [store: Store] }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()
const catalog = useWalletCatalog()

const isSub = computed(() => props.parent !== null)

/* --- step 1: what --- */
const step = ref<'pick' | 'details'>('pick')
const kind = ref<StoreKind>('bank')
const subType = ref<SubType | null>(null)

const creatableKinds = computed(() => (catalog.data.value?.kinds ?? []).filter((info) => info.user_creatable))
const subTypes = computed(() => (props.parent ? (kindInfo(catalog.data.value, props.parent.kind)?.sub_types ?? []) : []))
const subInfo = computed(() => subTypes.value.find((info) => info.sub_type === subType.value) ?? null)

/* --- step 2: details --- */
const name = ref('')
const nameTouched = ref(false)
const color = ref(DEFAULT_STORE_COLOR)
const terms = ref<LiabilityTerms>(emptyLiabilityTerms())
const custody = ref<Custody>('hot')
const provider = ref<ExchangeProvider | null>(null)
const propertyCategory = ref<PropertyCategory>('flat')
const securedBy = ref('')
const currency = ref('')
const token = ref<AssetRef | null>(null)
const pairText = ref('')

const pair = computed(() => parsePairSymbol(pairText.value))

/* --- optional opening balance: what the wallet already holds --- */
const openingAmount = ref('')
const openingCurrency = ref('')
const openingToken = ref<AssetRef | null>(null)
const openingSide = ref<'base' | 'quote'>('quote')

/** Property is valued by purchase and a loan by its terms: no opening balance there. */
const offersOpening = computed(() => !['property', 'liability'].includes(kind.value))

/** Which asset the opening balance is in, from the binding or the picker. */
const openingAsset = computed<AssetRef | null>(() => {
  const current = binding.value
  if (current && typeof current === 'object' && 'asset' in current) return current.asset
  if (current && typeof current === 'object' && 'pair' in current) return current.pair[openingSide.value]
  if (kind.value === 'bank' || kind.value === 'cash') return openingCurrency.value ? fiatRef(openingCurrency.value) : null
  return openingToken.value
})
/** The binding fixes the asset; otherwise the user picks it. */
const openingPicker = computed<'none' | 'currency' | 'asset' | 'side'>(() => {
  const current = binding.value
  if (current && typeof current === 'object' && 'asset' in current) return 'none'
  if (current && typeof current === 'object' && 'pair' in current) return 'side'
  return kind.value === 'bank' || kind.value === 'cash' ? 'currency' : 'asset'
})
const openingAmountValue = computed(() => openingAmount.value.trim().replace(',', '.'))
const openingValid = computed(
  () => !openingAmountValue.value || (Number(openingAmountValue.value) > 0 && openingAsset.value !== null),
)

const properties = useQuery({
  queryKey: STORE_TREE_QUERY_KEY,
  queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
  enabled: computed(() => open.value && kind.value === 'liability'),
})
const propertyRoots = computed(() =>
  (properties.data.value ?? []).filter((node) => node.store.kind === 'property' && !node.store.archived),
)

/** What the sub-account is locked to, from the fields of its shape. */
const binding = computed<Binding | null>(() => {
  const info = subInfo.value
  if (!info) return null
  if (info.shape === 'market') return null // the backend fills the market in
  if (info.shape === 'pair') {
    return pair.value ? { pair: { base: tickerRef(pair.value.base), quote: tickerRef(pair.value.quote) } } : null
  }
  if (props.parent?.kind === 'crypto') return token.value ? { asset: token.value } : null
  return currency.value ? { asset: fiatRef(currency.value) } : null
})

/** A sub-account name follows its type and binding until the user types one. */
const suggestedName = computed(() => {
  if (!subType.value) return ''
  const label = t(`subType.${subType.value}`)
  if (subInfo.value?.shape === 'pair' && pair.value) return `${pair.value.base}${pair.value.quote}`
  const current = binding.value
  const symbol = current && typeof current === 'object' && 'asset' in current ? current.asset.symbol : ''
  return symbol ? `${label} ${symbol}` : label
})
watch(suggestedName, (value) => {
  if (!nameTouched.value) name.value = value
})

const title = computed(() => {
  if (!props.parent) return step.value === 'pick' ? t('storeWizard.pickKind') : t('wallet.newStore')
  const path = props.parentPath.length ? props.parentPath : [props.parent.name]
  return t('stores.newSubAccount', { parent: path.join(' › ') })
})

watch(open, (isOpen) => {
  if (!isOpen) return
  name.value = ''
  nameTouched.value = false
  terms.value = emptyLiabilityTerms()
  custody.value = 'hot'
  provider.value = null
  propertyCategory.value = 'flat'
  securedBy.value = ''
  currency.value = settings.baseCurrency.toLowerCase()
  token.value = null
  pairText.value = ''
  subType.value = null
  openingAmount.value = ''
  openingCurrency.value = settings.baseCurrency.toLowerCase()
  openingToken.value = null
  openingSide.value = 'quote'
  if (props.parent) {
    kind.value = props.parent.kind
    color.value = props.parent.color
    step.value = 'pick'
  } else if (props.kind) {
    pickKind(props.kind)
  } else {
    step.value = 'pick'
  }
})

function pickKind(value: StoreKind) {
  kind.value = value
  color.value = KIND_COLORS[value] ?? DEFAULT_STORE_COLOR
  if (value === 'liability') terms.value = { ...emptyLiabilityTerms(), kind: 'loan' }
  step.value = 'details'
}

function pickSubType(value: SubType) {
  subType.value = value
  if (value === 'credit_card') terms.value = emptyLiabilityTerms()
  step.value = 'details'
}

const canGoBack = computed(() => step.value === 'details' && (isSub.value || !props.kind))

const valid = computed(() => {
  if (!name.value.trim() || !openingValid.value) return false
  if (isSub.value) return subInfo.value !== null && (subInfo.value.shape === 'market' || binding.value !== null)
  if (kind.value === 'exchange') return provider.value !== null
  return true
})

const createStore = useMutation({
  mutationFn: () =>
    call<Store>('create_store', {
      input: {
        name: name.value.trim(),
        kind: kind.value,
        color: color.value,
        note: '',
        parent_id: props.parent?.id ?? null,
        sub_type: isSub.value ? subType.value : null,
        binding: isSub.value ? binding.value : null,
        purpose: null,
        liability: kind.value === 'liability' || subInfo.value?.credit ? terms.value : null,
        contact_id: null,
        custody: !isSub.value && kind.value === 'crypto' ? custody.value : null,
        property:
          !isSub.value && kind.value === 'property'
            ? {
                category: propertyCategory.value,
                address: '',
                area: null,
                purchased_at: null,
                purchase_price: null,
                currency: settings.baseCurrency.toLowerCase(),
              }
            : null,
        secured_by: kind.value === 'liability' && securedBy.value ? securedBy.value : null,
        provider: !isSub.value && kind.value === 'exchange' ? provider.value : null,
        opening:
          offersOpening.value && openingAmountValue.value && openingAsset.value
            ? { asset: openingAsset.value, amount: openingAmountValue.value }
            : null,
      } satisfies CreateStoreInput,
    }),
  onSuccess: (store) => {
    toast.success(props.parent ? t('stores.subAccountCreated') : t('stores.created'), { description: store.name })
    for (const queryKey of [['stores'], ['store'], ['summary'], ['net-worth'], ['transactions'], ['history']]) {
      void queryClient.invalidateQueries({ queryKey })
    }
    open.value = false
    emit('created', store)
  },
  onError: (err) => toast.error(t('stores.createFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

const cardClass =
  'flex items-start gap-3 rounded-lg border border-border p-3 text-left transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50'
</script>

<template>
  <Dialog v-model:open="open" :title="title" size="sm">
    <!-- Step 1: a card per kind (top level) or per sub-account type. -->
    <div v-if="step === 'pick'" class="grid gap-2" data-tour="store-create-pick">
      <template v-if="!isSub">
        <button
          v-for="info in creatableKinds"
          :key="info.kind"
          type="button"
          :class="cardClass"
          @click="pickKind(info.kind)"
        >
          <component :is="getStoreIcon(info.kind)" class="mt-0.5 size-5 shrink-0" :style="{ color: KIND_COLORS[info.kind] }" />
          <span class="grid gap-0.5">
            <span class="text-sm font-medium">{{ t(`stores.kind.${info.kind}`) }}</span>
            <span class="text-xs text-muted-foreground">{{ t(`stores.kindHint.${info.kind}`) }}</span>
          </span>
        </button>
      </template>
      <template v-else>
        <p v-if="!subTypes.length" class="text-sm text-muted-foreground">{{ t('errors.SUB_TYPE_NOT_ALLOWED') }}</p>
        <button
          v-for="info in subTypes"
          :key="info.sub_type"
          type="button"
          :class="cardClass"
          @click="pickSubType(info.sub_type)"
        >
          <component :is="getSubTypeIcon(info.sub_type)" class="mt-0.5 size-5 shrink-0 text-muted-foreground" />
          <span class="grid gap-0.5">
            <span class="text-sm font-medium">{{ t(`subType.${info.sub_type}`) }}</span>
            <span class="text-xs text-muted-foreground">{{ t(`subTypeHint.${info.sub_type}`) }}</span>
          </span>
        </button>
      </template>
    </div>

    <!-- Step 2: only the fields this kind or sub-account type needs. -->
    <form v-else id="store-create-form" class="@container grid gap-4" @submit.prevent="valid && createStore.mutate()">
      <template v-if="isSub && subInfo">
        <div v-if="subInfo.shape === 'asset'" class="grid gap-1.5">
          <Label :for="props.parent?.kind === 'crypto' ? undefined : 'store-binding-currency'">
            {{ props.parent?.kind === 'crypto' ? t('binding.token') : t('binding.currency') }}
          </Label>
          <AssetInput v-if="props.parent?.kind === 'crypto'" v-model="token" />
          <CurrencySelect v-else v-model="currency" />
        </div>
        <div v-else-if="subInfo.shape === 'pair'" class="grid gap-1.5">
          <Label for="store-binding-pair">{{ t('binding.pair') }}</Label>
          <Input id="store-binding-pair" v-model="pairText" :placeholder="t('binding.pairPlaceholder')" autocomplete="off" />
          <p class="text-xs" :class="pairText && !pair ? 'text-destructive' : 'text-muted-foreground'">
            {{ pair ? t('binding.pairParsed', pair) : pairText ? t('binding.pairInvalid') : t('subTypeHint.pair') }}
          </p>
        </div>
        <p v-else class="text-sm text-muted-foreground">{{ t(`subTypeHint.${subInfo.sub_type}`) }}</p>
      </template>

      <template v-if="!isSub && kind === 'exchange'">
        <div class="grid gap-1.5">
          <Label>{{ t('provider.title') }}</Label>
          <div class="grid gap-2 @sm:grid-cols-3">
            <button
              v-for="info in catalog.data.value?.providers ?? []"
              :key="info.provider"
              type="button"
              :disabled="!info.supported"
              :aria-pressed="provider === info.provider"
              :class="cn(cardClass, 'items-center justify-between', provider === info.provider && 'border-primary bg-primary/5')"
              @click="provider = info.provider"
            >
              <span class="text-sm font-medium">{{ t(`provider.${info.provider}`) }}</span>
              <span v-if="!info.supported" class="text-xs text-muted-foreground">{{ t('provider.soon') }}</span>
            </button>
          </div>
        </div>
      </template>

      <div class="grid grid-cols-[1fr_auto] gap-4">
        <div class="grid gap-1.5">
          <Label for="store-name">{{ t('stores.name') }}</Label>
          <Input
            id="store-name"
            v-model="name"
            :placeholder="t('stores.namePlaceholder')"
            @input="nameTouched = true"
          />
        </div>
        <div class="grid gap-1.5">
          <Label for="store-color">{{ t('stores.color') }}</Label>
          <input
            id="store-color"
            v-model="color"
            type="color"
            class="h-9 w-14 cursor-pointer rounded-md border border-input bg-transparent p-1"
          >
        </div>
      </div>

      <div v-if="!isSub && kind === 'crypto'" class="grid gap-1.5">
        <Label>{{ t('custody.title') }}</Label>
        <ToggleGroup v-model="custody" type="single" variant="outline" size="sm">
          <ToggleGroupItem value="hot" class="px-3">{{ t('custody.hot') }}</ToggleGroupItem>
          <ToggleGroupItem value="cold" class="px-3">{{ t('custody.cold') }}</ToggleGroupItem>
        </ToggleGroup>
      </div>

      <div v-if="!isSub && kind === 'property'" class="grid gap-1.5">
        <Label for="store-property-category">{{ t('propertyCategory.title') }}</Label>
        <Select id="store-property-category" v-model="propertyCategory">
          <option v-for="entry in ['flat', 'house', 'car', 'land', 'other']" :key="entry" :value="entry">
            {{ t(`propertyCategory.${entry}`) }}
          </option>
        </Select>
      </div>

      <!-- Optional: money already there, so the new wallet does not start at zero. -->
      <fieldset v-if="offersOpening" class="grid gap-1.5" data-tour="store-opening">
        <Label for="store-opening-amount">{{ t('storeWizard.opening') }}</Label>
        <div class="grid grid-cols-[1fr_10rem] gap-2">
          <Input
            id="store-opening-amount"
            v-model="openingAmount"
            inputmode="decimal"
            autocomplete="off"
            :placeholder="t('storeWizard.openingPlaceholder')"
          />
          <CurrencySelect v-if="openingPicker === 'currency'" v-model="openingCurrency" />
          <AssetInput v-else-if="openingPicker === 'asset'" v-model="openingToken" />
          <Select v-else-if="openingPicker === 'side' && pair" v-model="openingSide" :aria-label="t('binding.pair')">
            <option value="base">{{ pair.base }}</option>
            <option value="quote">{{ pair.quote }}</option>
          </Select>
          <span v-else-if="openingAsset" class="flex h-9 items-center px-1 text-sm text-muted-foreground">
            {{ openingAsset.symbol }}
          </span>
        </div>
        <p class="text-xs" :class="openingValid ? 'text-muted-foreground' : 'text-destructive'">
          {{ openingValid ? t('storeWizard.openingHint') : t('storeWizard.openingInvalid') }}
        </p>
      </fieldset>

      <template v-if="kind === 'liability' || subInfo?.credit">
        <LiabilityTermsFields v-model="terms" />
        <div v-if="kind === 'liability' && propertyRoots.length" class="grid gap-1.5">
          <Label for="store-secured-by">{{ t('property.securedBy') }}</Label>
          <Select id="store-secured-by" v-model="securedBy">
            <option value="">{{ t('property.securedByNone') }}</option>
            <option v-for="node in propertyRoots" :key="node.store.id" :value="node.store.id">{{ node.store.name }}</option>
          </Select>
        </div>
      </template>
    </form>

    <template #footer>
      <Button v-if="canGoBack" variant="ghost" class="mr-auto" @click="step = 'pick'">
        <ArrowLeft class="size-4" />
        {{ t('storeWizard.back') }}
      </Button>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button
        v-if="step === 'details'"
        form="store-create-form"
        type="submit"
        :disabled="!valid || createStore.isPending.value"
      >
        <Plus class="size-4" />
        {{ t('stores.create') }}
      </Button>
    </template>
  </Dialog>
</template>
