import { createApp } from 'vue';
import PrimeVue from 'primevue/config';
import Aura from '@primeuix/themes/aura';
import App from './App.vue';
import './style.css';

document.documentElement.classList.add('dark');

createApp(App).use(PrimeVue, {
  theme: { preset: Aura, options: { darkModeSelector: '.dark' } },
}).mount('#app');
