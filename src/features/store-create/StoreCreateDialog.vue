<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ArrowLeft, Plus } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'

import { useCreateStore } from './model/use-create-store'
import { provideStoreCreate } from './model/use-store-create-state'
import type { StoreCreateDialogProps } from './store-create.types'
import { ExchangeProviderFields } from './ui/exchange-provider-fields'
import { StoreBindingFields } from './ui/store-binding-fields'
import { StoreKindFields } from './ui/store-kind-fields'
import { StoreKindPicker } from './ui/store-kind-picker'
import { StoreMoneyFields } from './ui/store-money-fields'

/**
 * A new wallet or sub-account in two steps: what it is (a card per kind or
 * type), then only the fields that kind needs. State and checks live in
 * `model/use-store-create-state`, the sections in `ui/`.
 */
const props = withDefaults(defineProps<StoreCreateDialogProps>(), { parent: null, parentPath: () => [], kind: null })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ created: [store: Store] }>()

const { t } = useI18n()
const state = provideStoreCreate(props, open)
const { step, isSub, kind, name, nameTouched, color, title, canGoBack, form } = state
const createStore = useCreateStore(state, (store) => emit('created', store))
</script>

<template>
  <Dialog v-model:open="open" :title="title" size="sm">
    <StoreKindPicker v-if="step === 'pick'" />

    <!-- Step 2: only the fields this kind or sub-account type needs. -->
    <form v-else id="store-create-form" class="@container grid gap-4" @submit.prevent="form.submit(createStore.mutate, $event)">
      <StoreBindingFields v-if="isSub" />
      <ExchangeProviderFields v-if="!isSub && kind === 'exchange'" />

      <div class="grid grid-cols-[1fr_auto] gap-4">
        <FormField for="store-name" :label="t('stores.name')" required :error="form.error('name')">
          <Input id="store-name" v-model="name" :placeholder="t('stores.namePlaceholder')" @input="nameTouched = true" />
        </FormField>
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

      <StoreKindFields />
      <StoreMoneyFields />
    </form>

    <template #footer>
      <Button v-if="canGoBack" variant="ghost" class="mr-auto" @click="step = 'pick'">
        <ArrowLeft class="size-4" />
        {{ t('storeWizard.back') }}
      </Button>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button v-if="step === 'details'" form="store-create-form" type="submit" :disabled="createStore.isPending.value" :loading="createStore.isPending.value">
        <Plus class="size-4" />
        {{ t('stores.create') }}
      </Button>
    </template>
  </Dialog>
</template>
