<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Plus } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'

/** «+ Contact»: the page's one main action; the shell header already names the page. */
const emit = defineEmits<{ create: [] }>()
const { t } = useI18n()
</script>

<template>
  <Button variant="outline" data-tour="contact-quick-create" @click="emit('create')">
    <Plus class="size-4" />{{ t('contacts.add') }}
  </Button>
</template>
