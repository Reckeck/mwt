<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, X } from 'lucide-vue-next'

import { cardBrand } from '@/entities/contact'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { CHAIN_LABELS, CRYPTO_CHAINS } from '@/shared/lib/chains'
import { detectDetail } from '@/shared/lib/detect-detail'
import { required, useFormValidation } from '@/shared/lib/form'

import type { ContactAccountFormProps } from '../contact-edit.types'

/**
 * A requisite before it is saved (Ops-2 · 8): the pasted value, what the app
 * made of it, a label and a group. Enter saves, Esc drops it. A crypto address
 * whose network was not recognised cannot be saved without one.
 */
const props = defineProps<ContactAccountFormProps>()
const emit = defineEmits<{ submit: [account: ContactAccount]; cancel: [] }>()

const { t } = useI18n()

const value = ref(props.initial?.value ?? props.value ?? '')
const label = ref(props.initial?.label ?? '')
const group = ref(props.initial?.group ?? '')
const guess = computed(() => detectDetail(value.value))
const network = ref<Chain | ''>(props.initial?.network ?? '')
const kind = computed(() => (props.initial && props.initial.value === value.value ? props.initial.kind : guess.value.kind))
const shownNetwork = computed(() => network.value || guess.value.network || '')
/** Mir cards are rouble cards; other currencies are kept from the stored requisite. */
const currency = computed(() => {
  if (props.initial?.currency) return props.initial.currency
  return kind.value === 'card' && cardBrand(value.value) === 'mir' ? 'rub' : null
})
const needsNetwork = computed(() => kind.value === 'crypto_address' && !guess.value.network)

const form = useFormValidation(() => ({
  value: required(value.value),
  network: needsNetwork.value && !network.value ? t('contacts.networkRequired') : null,
}))

/** «карта Мир · RUB», «криптоадрес · TRON». */
const recognized = computed(() => {
  if (!value.value.trim()) return ''
  const brand = kind.value === 'card' ? cardBrand(value.value) : null
  return [
    t(`detailKind.${kind.value}`).toLowerCase() + (brand ? ` ${t(`cardBrand.${brand}`)}` : ''),
    shownNetwork.value && CHAIN_LABELS[shownNetwork.value as Chain].toUpperCase(),
    currency.value?.toUpperCase(),
  ]
    .filter(Boolean)
    .join(' · ')
})

function save() {
  emit('submit', {
    id: props.initial?.id ?? '',
    group: group.value.trim() || null,
    label: label.value.trim(),
    kind: kind.value,
    value: kind.value === guess.value.kind ? guess.value.value : value.value.trim(),
    currency: currency.value,
    network: (shownNetwork.value as Chain) || null,
    active: props.initial?.active ?? true,
  })
}

const valueInput = ref<InstanceType<typeof Input> | null>(null)
onMounted(() => {
  void nextTick(() => (valueInput.value?.$el as HTMLInputElement | undefined)?.focus())
})
</script>

<template>
  <form
    class="grid gap-3 rounded-lg border border-primary bg-card p-3"
    data-tour="contact-account-form"
    @submit.prevent="form.submit(save, $event)"
    @keydown.esc.stop.prevent="emit('cancel')"
  >
    <div class="grid gap-1.5" :data-invalid="form.invalidAttr('value')">
      <div class="relative">
        <Input
          id="contact-account-value"
          ref="valueInput"
          v-model="value"
          class="pr-10 tabular-nums"
          spellcheck="false"
          autocomplete="off"
          :placeholder="t('contacts.pastePlaceholder')"
          :aria-label="t('contacts.accountValue')"
        />
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          class="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground"
          :title="t('actions.cancel')"
          :aria-label="t('actions.cancel')"
          @click="emit('cancel')"
        >
          <X class="size-4" />
        </Button>
      </div>
      <p v-if="form.error('value')" class="text-xs text-destructive" role="alert">{{ form.error('value') }}</p>
      <p v-else-if="recognized" class="flex items-center gap-2 text-xs text-primary">
        <Check class="size-4" />{{ t('contacts.recognized', { what: recognized }) }}
      </p>
    </div>

    <div class="grid gap-3 @md:grid-cols-2">
      <FormField for="contact-account-label" :label="t('contacts.accountTitle')">
        <Input id="contact-account-label" v-model="label" autocomplete="off" :placeholder="t(`detailKind.${kind}`)" />
      </FormField>
      <FormField for="contact-account-group" :label="t('contacts.group')">
        <Input id="contact-account-group" v-model="group" list="contact-account-groups" autocomplete="off" />
        <datalist id="contact-account-groups">
          <option v-for="entry in props.groups" :key="entry" :value="entry" />
        </datalist>
      </FormField>
      <FormField
        v-if="kind === 'crypto_address'"
        :label="t('form.network')"
        :required="needsNetwork"
        :error="form.error('network')"
      >
        <Select v-model="network" :aria-label="t('form.network')">
          <SelectItem v-for="chain in CRYPTO_CHAINS" :key="chain" :value="chain">{{ CHAIN_LABELS[chain] }}</SelectItem>
        </Select>
      </FormField>
    </div>

    <div class="flex flex-wrap items-center justify-end gap-2">
      <span class="mr-auto text-xs text-muted-foreground pointer-coarse:hidden">Enter · Esc</span>
      <Button type="button" variant="outline" @click="emit('cancel')">{{ t('actions.cancel') }}</Button>
      <Button type="submit" :loading="props.pending" :disabled="props.pending">
        {{ props.initial ? t('actions.save') : t('actions.add') }}
      </Button>
    </div>
  </form>
</template>
