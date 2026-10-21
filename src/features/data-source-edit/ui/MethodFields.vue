<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'

import type { SourceDraftState, SourceField } from '../model/use-source-draft'

/** How to find the value: the method and its target, editable after a pick. */
const props = defineProps<{ state: SourceDraftState; error: (field: SourceField) => string | undefined }>()
const { t } = useI18n()

const METHODS = ['css', 'table', 'json', 'label', 'file'] as const
const ROWS = ['by_date', 'first', 'last'] as const
const OUTPUTS = ['number', 'series', 'text'] as const
</script>

<template>
  <div class="grid gap-4 sm:grid-cols-2" data-tour="source-method">
    <FormField for="source-method" :label="t('sources.methodLabel')">
      <Select id="source-method" v-model="props.state.kind.value">
        <SelectItem v-for="method in METHODS" :key="method" :value="method">
          {{ t(`sources.method.${method}`) }}
        </SelectItem>
      </Select>
    </FormField>
    <FormField for="source-output" :label="t('sources.output.label')">
      <Select id="source-output" v-model="props.state.output.value">
        <SelectItem v-for="output in OUTPUTS" :key="output" :value="output">
          {{ t(`sources.output.${output}`) }}
        </SelectItem>
      </Select>
    </FormField>

    <FormField
      v-if="props.state.kind.value === 'css' || props.state.kind.value === 'table'"
      for="source-selector"
      :label="t(props.state.kind.value === 'table' ? 'sources.tableSelector' : 'sources.selector')"
      :hint="t('sources.selectorHint')"
      required
      :error="props.error('target')"
      class="sm:col-span-2"
    >
      <Input id="source-selector" v-model="props.state.selector.value" class="font-mono" spellcheck="false" />
    </FormField>
    <FormField
      v-if="props.state.kind.value === 'css'"
      for="source-hint"
      :label="t('sources.textHint')"
      :hint="t('sources.textHintHint')"
      class="sm:col-span-2"
    >
      <Input id="source-hint" v-model="props.state.textHint.value" />
    </FormField>
    <FormField
      v-if="props.state.kind.value === 'json'"
      for="source-path"
      :label="t('sources.jsonPath')"
      :hint="t('sources.jsonPathHint')"
      required
      :error="props.error('target')"
      class="sm:col-span-2"
    >
      <Input id="source-path" v-model="props.state.path.value" class="font-mono" spellcheck="false" />
    </FormField>
    <FormField
      v-if="props.state.kind.value === 'label'"
      for="source-label"
      :label="t('sources.labelText')"
      :hint="t('sources.labelTextHint')"
      required
      :error="props.error('target')"
      class="sm:col-span-2"
    >
      <Input id="source-label" v-model="props.state.label.value" />
    </FormField>
    <template v-if="props.state.kind.value === 'file'">
      <FormField for="source-format" :label="t('sources.fileFormat')">
        <Select id="source-format" v-model="props.state.fileFormat.value">
          <SelectItem value="csv">CSV</SelectItem>
          <SelectItem value="xlsx">XLSX</SelectItem>
        </Select>
      </FormField>
      <FormField
        v-if="props.state.fileFormat.value === 'xlsx'"
        for="source-sheet"
        :label="t('sources.sheet')"
        :hint="t('sources.sheetHint')"
      >
        <Input id="source-sheet" v-model="props.state.sheet.value" />
      </FormField>
    </template>
    <template v-if="props.state.kind.value === 'table' || props.state.kind.value === 'file'">
      <FormField
        for="source-column"
        :label="t('sources.column')"
        :hint="t('sources.columnHint')"
        required
        :error="props.error('column')"
      >
        <Input id="source-column" v-model="props.state.column.value" />
      </FormField>
      <FormField for="source-row" :label="t('sources.row.label')">
        <Select id="source-row" v-model="props.state.row.value">
          <SelectItem v-for="row in ROWS" :key="row" :value="row">{{ t(`sources.row.${row}`) }}</SelectItem>
        </Select>
      </FormField>
    </template>
  </div>
</template>
