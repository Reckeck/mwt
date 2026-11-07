<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check } from 'lucide-vue-next'

/** Step 1: what the move changes. No archive item yet: the capital warning arrives with the addresses (E11). */
const props = defineProps<{ level: SecurityLevel }>()
const { t } = useI18n()

const items = computed(() => [
  props.level === 'pin' ? 'passwordPin' : 'passwordPlain',
  'reencrypt',
  'autolock',
  'downgrade',
])
</script>

<template>
  <div class="grid gap-3">
    <p class="text-sm">{{ t('securityRaise.warning.lead', { level: props.level === 'pin' ? 2 : 1 }) }}</p>
    <ul class="grid gap-2 text-sm">
      <li v-for="item in items" :key="item" class="flex items-start gap-2">
        <Check class="mt-0.5 size-4 shrink-0 text-success" aria-hidden="true" />
        <span>{{ t(`securityRaise.warning.${item}`) }}</span>
      </li>
    </ul>
  </div>
</template>
