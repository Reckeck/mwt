<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowRight, Minus, PenLine, Plus } from 'lucide-vue-next'

import { Money } from '@/entities/currency'

import { collectionKey } from '../../lib/collections'
import { describeField } from '../../lib/fields'
import type { SyncChangeItemProps } from './sync-change-item.types'

/** One changed record: what it is, and for an edited one each field as `before → after`. */
const props = defineProps<SyncChangeItemProps>()
const { t, te } = useI18n()

const icons = { added: Plus, updated: PenLine, deleted: Minus } as const
const fields = computed(() => props.item.fields.map((field) => describeField(field, t, te)))
const title = computed(() => props.item.label || t(collectionKey(props.collection)))
</script>

<template>
  <li class="grid gap-0.5">
    <div class="flex items-center gap-2">
      <component :is="icons[props.item.kind]" class="size-3.5 shrink-0 text-muted-foreground" :aria-label="t(`sync.kind.${props.item.kind}`)" />
      <span class="min-w-0 truncate">{{ title }}</span>
    </div>
    <dl v-if="fields.length" class="ml-5 grid gap-0.5 text-xs">
      <div v-for="(field, index) in fields" :key="index" class="flex flex-wrap items-baseline gap-x-1.5">
        <dt class="text-muted-foreground">{{ field.label }}:</dt>
        <dd class="flex min-w-0 flex-wrap items-baseline gap-x-1.5">
          <template v-if="field.currency">
            <Money v-if="field.before !== null" :amount="field.before" :currency="field.currency" class="text-muted-foreground" />
            <span v-else class="text-muted-foreground">{{ t('sync.field.empty') }}</span>
            <ArrowRight class="size-3 shrink-0 self-center text-muted-foreground" aria-hidden="true" />
            <Money v-if="field.after !== null" :amount="field.after" :currency="field.currency" />
            <span v-else>{{ t('sync.field.empty') }}</span>
          </template>
          <template v-else>
            <span class="min-w-0 break-words text-muted-foreground">{{ field.before ?? t('sync.field.empty') }}</span>
            <ArrowRight class="size-3 shrink-0 self-center text-muted-foreground" aria-hidden="true" />
            <span class="min-w-0 break-words">{{ field.after ?? t('sync.field.empty') }}</span>
          </template>
        </dd>
      </div>
    </dl>
  </li>
</template>
