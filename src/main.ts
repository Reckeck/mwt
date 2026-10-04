import { createPinia } from 'pinia'
import { VueQueryPlugin } from '@tanstack/vue-query'
import { createApp } from 'vue'

import App from '@/App.vue'
import { i18n } from '@/i18n'
import { router } from '@/router'
import '@/index.css'

const app = createApp(App)

app.use(router)
app.use(createPinia())
app.use(VueQueryPlugin)
app.use(i18n)

app.mount('#app')
