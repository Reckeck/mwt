<script setup lang="ts">
import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { useFormValidation } from '@/shared/lib/form'
import { AdvancedFields } from '@/shared/ui/advanced-fields'
import { Button } from '@/shared/ui/button'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'

import { useSourceActions } from '../model/use-source-actions'
import { useSourceDraft } from '../model/use-source-draft'
import { useSourcePick } from '../model/use-source-pick'
import JsonPicker from './JsonPicker.vue'
import MethodFields from './MethodFields.vue'
import PagePicker from './PagePicker.vue'
import RowsPicker from './RowsPicker.vue'
import UsesFields from './UsesFields.vue'

/**
 * «Новый источник» and «Изменить» / «Починить»: the address, a click on the
 * value in the page as the background fetch sees it, «Проверить», then the
 * name, how often and where the value goes.
 */
const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{ view: DataSourceView | null }>()
const { t } = useI18n()

const state = useSourceDraft()
const actions = useSourceActions()
const pick = useSourcePick(state, actions)
const form = useFormValidation(() => state.rules(), { resetOn: open })

const INTERVALS = ['hour', 'day', 'week'] as const

watch(open, (value) => {
  if (!value) return
  state.load(props.view)
  pick.reset()
  if (props.view) void pick.open()
})

const resultText = computed(() => {
  const result = pick.result.value
  if (!result) return ''
  if (result.series.length) {
    const last = result.series[result.series.length - 1]
    return t('sources.testSeries', { count: result.series.length, label: last.label, value: last.value })
  }
  return result.number ?? result.text ?? t('sources.testEmpty')
})

function save() {
  actions.save.mutate(state.toDraft(), { onSuccess: () => (open.value = false) })
}
</script>

<template>
  <FormDialog overlay="source"
    v-model:open="open"
    :title="props.view ? t('sources.edit') : t('sources.add')"
    :description="t('sources.addHint')"
    size="lg"
    form-id="source-form"
    :submit-label="t('actions.save')"
    :pending="actions.save.isPending.value"
    @submit="form.submit(save, $event)"
  >
    <div class="grid gap-2 sm:grid-cols-[1fr_auto] sm:items-end" data-tour="source-url">
      <FormField for="source-url" :label="t('sources.url')" required :error="form.error('url') ?? pick.previewError.value ?? undefined">
        <Input id="source-url" v-model="state.url.value" type="url" inputmode="url" placeholder="https://" />
      </FormField>
      <Button
        type="button"
        variant="outline"
        :disabled="!state.url.value.trim()"
        :loading="actions.preview.isPending.value"
        @click="pick.open"
      >
        {{ t('sources.open') }}
      </Button>
    </div>

    <p v-if="props.view?.source.status === 'broken'" class="text-sm text-warning">
      {{ t('sources.fixHint', { error: props.view.source.error ?? '' }) }}
    </p>

    <template v-if="pick.preview.value">
      <PagePicker
        v-if="pick.preview.value.kind === 'html' && pick.preview.value.html"
        :html="pick.preview.value.html"
        :selected="pick.marked.value"
        @pick="pick.pickElement"
      />
      <JsonPicker
        v-else-if="pick.preview.value.kind === 'json' && pick.preview.value.json"
        :json="pick.preview.value.json"
        :selected="state.path.value"
        @pick="pick.pickPath"
      />
      <RowsPicker v-else :rows="pick.preview.value.rows" :selected="state.column.value" @pick="pick.pickColumn" />
    </template>

    <MethodFields :state="state" :error="form.error" />

    <div class="grid gap-2 sm:grid-cols-[auto_1fr] sm:items-center" data-tour="source-test">
      <Button type="button" variant="secondary" :loading="actions.test.isPending.value" @click="pick.test">
        {{ t('sources.test') }}
      </Button>
      <p class="min-h-5 text-sm" :class="pick.testError.value ? 'text-destructive' : 'text-foreground'" aria-live="polite">
        {{ pick.testError.value ?? resultText }}
      </p>
      <p v-if="pick.result.value?.warning" class="text-xs text-warning sm:col-span-2">
        {{ t('sources.testWarning', { warning: pick.result.value.warning }) }}
      </p>
    </div>

    <div class="grid gap-4 sm:grid-cols-2">
      <FormField for="source-name" :label="t('sources.name')" required :error="form.error('name')">
        <Input id="source-name" v-model="state.name.value" maxlength="64" :placeholder="t('sources.namePlaceholder')" />
      </FormField>
      <FormField for="source-interval" :label="t('sources.interval.label')" :hint="t('sources.interval.hint')">
        <Select id="source-interval" v-model="state.interval.value">
          <SelectItem v-for="interval in INTERVALS" :key="interval" :value="interval">
            {{ t(`sources.interval.${interval}`) }}
          </SelectItem>
        </Select>
      </FormField>
    </div>

    <AdvancedFields :label="t('sources.numberFormat')" :default-open="!!form.error('multiplier')">
      <div class="grid gap-4 sm:grid-cols-2">
        <SwitchRow
          id="source-comma"
          v-model="state.decimalComma.value"
          :label="t('sources.decimalComma')"
          :hint="t('sources.decimalCommaHint')"
          class="sm:col-span-2"
        />
        <FormField
          for="source-multiplier"
          :label="t('sources.multiplier')"
          :hint="t('sources.multiplierHint')"
          required
          :error="form.error('multiplier')"
        >
          <Input id="source-multiplier" v-model="state.multiplier.value" inputmode="decimal" />
        </FormField>
        <FormField for="source-strip" :label="t('sources.strip')" :hint="t('sources.stripHint')">
          <Input id="source-strip" v-model="state.strip.value" />
        </FormField>
      </div>
    </AdvancedFields>

    <UsesFields :state="state" />
  </FormDialog>
</template>
