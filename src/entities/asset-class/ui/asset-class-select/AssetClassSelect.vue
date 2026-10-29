<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Select, SelectItem } from '@/shared/ui/select'

import { assetClassName } from '../../lib'

/** A basket out of the user's list; `''` is `No basket` (or what `emptyLabel` says it means there). */
const props = defineProps<{ classes: AssetClass[]; disabled?: boolean; emptyLabel?: string }>()
const model = defineModel<string>({ default: '' })
const { t } = useI18n()
</script>

<template>
  <Select v-model="model" :disabled="props.disabled">
    <SelectItem value="">{{ props.emptyLabel ?? t('assetClass.none') }}</SelectItem>
    <SelectItem v-for="cls in props.classes" :key="cls.id" :value="cls.id">{{ assetClassName(cls, t) }}</SelectItem>
  </Select>
</template>
