<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowLeftRight, PiggyBank, Scale, TrendingUp, Wallet } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { RadioGroup, RadioGroupItem } from '@/shared/ui/radio-group'
import { Select, SelectItem } from '@/shared/ui/select'
import { Separator } from '@/shared/ui/separator'
import { SwitchRow } from '@/shared/ui/switch-row'
import { cn } from '@/shared/lib/utils'

import { useStoreSettings } from '../../model/use-store-settings'

/**
 * «Настройки · <кошелёк>» (Wallets-3 · 19): the four tabs of settings in one
 * screen — name and kind, budget, favourite, what the money is for, reconcile
 * and archive. The rarer settings stay on the page behind «Все настройки».
 */
const props = defineProps<{
  store: Store
  /** Kinds the wallet may become; empty when its kind is fixed. */
  kinds: StoreKind[]
}>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ adjust: []; changeKind: []; more: [] }>()
const { t } = useI18n()

const { name, purpose, favorite, inBudget, purposeEditable, form, save, pending, archive } = useStoreSettings(
  () => props.store,
  open,
  () => (open.value = false),
)

const PURPOSES: { value: Purpose; icon: typeof Wallet }[] = [
  { value: 'investment', icon: TrendingUp },
  { value: 'everyday', icon: Wallet },
  { value: 'savings', icon: PiggyBank },
  { value: 'transit', icon: ArrowLeftRight },
]
const kind = computed({
  get: () => props.store.kind,
  /** Another kind is a change with a preview and a confirmation: its own dialog. */
  set: (next: StoreKind) => {
    if (next === props.store.kind) return
    open.value = false
    emit('changeKind')
  },
})
const budgetHint = computed(() =>
  inBudget.value
    ? t('storeSettings.budgetOn')
    : t('storeSettings.budgetOff', { kind: t(`stores.kind.${props.store.kind}`), purpose: t(`purpose.${purpose.value}`).toLowerCase() }),
)
function leave(event: 'adjust' | 'more') {
  open.value = false
  if (event === 'adjust') emit('adjust')
  else emit('more')
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="t('storeSettings.title', { name: props.store.name })"
    size="bill"
    form-id="store-settings"
    :pending="pending"
    phone-submit-only
    @submit="form.submit(save, $event)"
  >
    <div class="grid gap-3.5 @md:grid-cols-2">
      <FormField for="store-settings-name" :label="t('stores.name')" required :error="form.error('name')">
        <Input id="store-settings-name" v-model="name" />
      </FormField>
      <FormField for="store-settings-kind" :label="t('stores.kind')">
        <Select id="store-settings-kind" v-model="kind" :disabled="!props.kinds.length">
          <SelectItem :value="props.store.kind">{{ t(`stores.kind.${props.store.kind}`) }}</SelectItem>
          <SelectItem v-for="entry in props.kinds" :key="entry" :value="entry">{{ t(`stores.kind.${entry}`) }}</SelectItem>
        </Select>
      </FormField>
    </div>

    <SwitchRow v-if="purposeEditable" id="store-settings-budget" v-model="inBudget" plain :label="t('storeSettings.budget')" :hint="budgetHint" />
    <SwitchRow id="store-settings-favorite" v-model="favorite" plain :label="t('storeSettings.favorite')" />

    <template v-if="purposeEditable">
      <Separator />
      <fieldset class="grid gap-3.5" data-tour="wallet-purpose">
        <legend class="mb-3.5 text-sm font-semibold">{{ t('purpose.title') }}</legend>
        <RadioGroup v-model="purpose" class="grid grid-cols-2 gap-3">
          <label
            v-for="entry in PURPOSES"
            :key="entry.value"
            :for="`store-settings-purpose-${entry.value}`"
            :class="
              cn(
                'relative flex cursor-pointer flex-col gap-1 rounded-[10px] border border-border p-3 transition-colors hover:bg-accent',
                'has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring',
                purpose === entry.value && 'border-primary ring-1 ring-primary hover:bg-transparent',
              )
            "
            :data-purpose="entry.value"
          >
            <span class="sr-only"><RadioGroupItem :id="`store-settings-purpose-${entry.value}`" :value="entry.value" /></span>
            <span class="flex items-center gap-2 text-sm font-medium">
              <component :is="entry.icon" class="size-4 shrink-0" />{{ t(`purpose.${entry.value}`) }}
            </span>
            <span class="text-xs text-muted-foreground">{{ t(`purpose.${entry.value}Short`) }}</span>
          </label>
        </RadioGroup>
      </fieldset>
    </template>

    <Separator />
    <div class="flex flex-wrap items-center gap-2">
      <Button v-if="props.store.kind !== 'property' && !props.store.archived" type="button" variant="outline" data-settings-adjust @click="leave('adjust')">
        <Scale class="size-4" />{{ t('adjustment.open') }}
      </Button>
      <Button
        type="button"
        variant="outline"
        class="ms-auto"
        :disabled="archive.isPending.value"
        data-settings-archive
        @click="archive.mutate(!props.store.archived)"
      >
        {{ props.store.archived ? t('stores.unarchive') : t('storeSettings.archive') }}
      </Button>
      <Button type="button" variant="link" class="h-auto w-full px-0 md:hidden" @click="leave('more')">{{ t('storeSettings.more') }}</Button>
    </div>

    <template #footer-start>
      <Button type="button" variant="link" class="h-auto px-0 max-md:hidden" data-settings-more @click="leave('more')">
        {{ t('storeSettings.more') }}
      </Button>
    </template>
  </FormDialog>
</template>
