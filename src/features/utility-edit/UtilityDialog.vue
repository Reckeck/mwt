<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Combobox } from '@/shared/ui/combobox'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { SyncChangedBanner } from '@/shared/ui/sync-changed-banner'
import { WizardSteps } from '@/shared/ui/wizard-steps'
import { useCategories } from '@/shared/api/queries'
import { categoryLabel } from '@/shared/lib/categories'

import { useSaveUtility } from './model/use-save-utility'
import { provideUtilityForm } from './model/use-utility-form'
import { MeterFields } from './ui/meter-fields'
import { RentFields } from './ui/rent-fields'
import { UtilityKindStep } from './ui/utility-kind-step'
import type { UtilityDialogProps } from './utility-edit.types'

/**
 * A bill item in two steps (Wallets-3 · 14–17): its name and how it is counted,
 * then the fields of that kind with the wallet and the category. Editing is
 * one form — step 2 with the name on top. State lives in `model/`.
 */
const props = withDefaults(defineProps<UtilityDialogProps>(), { item: null, propertyId: '' })
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()

const state = provideUtilityForm(props, open)
const { editing, step, name, kind, propertyName, storeId, category, nodes, form, changedElsewhere, refreshFromOtherDevice } =
  state
const save = useSaveUtility(props, state, () => (open.value = false))

const storeOptions = computed(() => nodes.value.map((node) => ({ value: node.store.id, label: node.store.name })))
const categories = useCategories()
const categoryOptions = computed(() =>
  (categories.data.value ?? [])
    .filter((entry) => entry.group === 'fixed')
    .map((entry) => ({ value: entry.key, label: categoryLabel(entry, t) })),
)

const secondTitle = computed(() => t(`utilityEdit.step.${kind.value}`))
const title = computed(() => {
  const own = editing.value ? (props.item?.name ?? '') : name.value.trim()
  const head = step.value === 1 || !own ? t('utilityEdit.newTitle') : own
  return propertyName.value ? `${head} · ${propertyName.value}` : head
})

/** Step 1 checks the name and moves on; step 2 saves. */
function submit(event?: Event) {
  form.submit(() => {
    if (step.value === 1) step.value = 2
    else save.mutate()
  }, event)
}
</script>

<template>
  <Dialog overlay="utility" v-model:open="open" :title="title" size="bill">
    <form id="utility-form" class="@container grid gap-4" @submit.prevent="submit($event)">
      <SyncChangedBanner v-if="changedElsewhere" @refresh="refreshFromOtherDevice" />
      <WizardSteps v-if="!editing" :step="step" :titles="[t('utilityEdit.step.kind'), secondTitle]" />
      <UtilityKindStep v-if="step === 1" :ask-property="!props.propertyId" @next="submit()" />
      <template v-else>
        <FormField v-if="editing" for="utility-name" :label="t('utilities.name')" required :error="form.error('name')">
          <Input id="utility-name" v-model="name" />
        </FormField>
        <MeterFields v-if="kind === 'metered'" />
        <RentFields v-else />
        <div class="grid gap-3 @lg:grid-cols-2">
          <div class="grid content-start gap-1.5">
            <Label for="utility-store">{{ t('utilityEdit.store') }}</Label>
            <Combobox id="utility-store" v-model="storeId" :options="storeOptions" :empty-label="t('utilities.anyStore')" :placeholder="t('utilities.anyStore')" />
          </div>
          <div class="grid content-start gap-1.5">
            <Label for="utility-category">{{ t('utilities.category') }}</Label>
            <Combobox id="utility-category" v-model="category" :options="categoryOptions" :placeholder="t('utilities.category')" />
          </div>
        </div>
      </template>
    </form>
    <template #footer>
      <Button
        variant="outline"
        :class="['max-md:h-11', (editing || step === 1) && 'max-md:hidden']"
        @click="!editing && step === 2 ? (step = 1) : (open = false)"
      >
        {{ !editing && step === 2 ? t('actions.back') : t('actions.cancel') }}
      </Button>
      <Button type="submit" form="utility-form" class="max-md:h-11 max-md:flex-1"
        :loading="save.isPending.value"
        :data-wizard-next="step === 1 || undefined"
        :data-wizard-save="step === 2 || undefined">
        {{ step === 1 ? t('actions.next') : t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
