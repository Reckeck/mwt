<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { LiabilityTermsFields, PROPERTY_CATEGORIES, StoreNodeSelect } from '@/entities/store'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { useStoreCreate } from '../../model/use-store-create-state'

/** What only some kinds ask: crypto custody, property category, loan terms and collateral. */
const { t } = useI18n()
const { isSub, kind, subInfo, custody, propertyCategory, terms, propertyRoots, securedBy } = useStoreCreate()
</script>

<template>
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
      <SelectItem v-for="entry in PROPERTY_CATEGORIES" :key="entry" :value="entry">
        {{ t(`propertyCategory.${entry}`) }}
      </SelectItem>
    </Select>
  </div>

  <template v-if="kind === 'liability' || subInfo?.credit">
    <LiabilityTermsFields v-model="terms" />
    <div v-if="kind === 'liability' && propertyRoots.length" class="grid gap-1.5">
      <Label for="store-secured-by">{{ t('property.securedBy') }}</Label>
      <StoreNodeSelect id="store-secured-by" v-model="securedBy" :nodes="propertyRoots" flat :empty-label="t('property.securedByNone')" />
    </div>
  </template>
</template>
