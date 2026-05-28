<template>
  <div ref="menuRef" class="settings-menu-wrapper">
    <button
      class="settings-trigger"
      :class="{ open: isOpen }"
      :title="t('settings.title')"
      @click="isOpen = !isOpen"
    >
      <Settings :size="16" />
    </button>
    <transition name="fade">
      <div v-if="isOpen" class="settings-panel">
        <!-- Mode Section -->
        <div class="settings-section">
          <div class="settings-section-title">{{ t('settings.mode') }}</div>
          <div class="mode-toggle">
            <button
              class="mode-btn"
              :class="{ active: forgeMode === 'gsd' }"
              :title="t('settings.modeGsdTitle')"
              @click="setForgeMode('gsd')"
            >
              GSD
            </button>
            <button
              class="mode-btn"
              :class="{ active: forgeMode === 'check' }"
              :title="t('settings.modeCheckTitle')"
              @click="setForgeMode('check')"
            >
              Check
            </button>
          </div>
        </div>

        <!-- Accent Section -->
        <div class="settings-section">
          <div class="settings-section-title">{{ t('settings.accent') }}</div>
          <div class="accent-swatches">
            <button
              v-for="opt in accentOptions"
              :key="opt.name"
              class="accent-swatch"
              :class="{ active: accentCurrent === opt.name }"
              :style="{ background: opt.brand1 }"
              :title="opt.label"
              @click="setAccent(opt.name)"
            >
              <Check v-if="accentCurrent === opt.name" :size="12" />
            </button>
          </div>
        </div>

        <!-- Theme Section -->
        <div class="settings-section">
          <div class="settings-section-title">{{ t('settings.theme') }}</div>
          <div class="theme-options">
            <button
              v-for="opt in themeOptions"
              :key="opt.value"
              class="theme-option"
              :class="{ active: themeMode === opt.value }"
              @click="setMode(opt.value)"
            >
              <component :is="opt.icon" :size="14" />
              <span>{{ opt.label }}</span>
              <Check v-if="themeMode === opt.value" :size="12" class="check" />
            </button>
          </div>
        </div>

        <!-- Language Section -->
        <div class="settings-section">
          <div class="settings-section-title">{{ t('settings.language') }}</div>
          <div class="language-options">
            <button
              class="language-option"
              :class="{ active: currentLocale === 'en' }"
              @click="changeLocale('en')"
            >
              <span class="lang-code">EN</span>
              <span class="lang-name">English</span>
              <Check v-if="currentLocale === 'en'" :size="12" class="check" />
            </button>
            <button
              class="language-option"
              :class="{ active: currentLocale === 'zh' }"
              @click="changeLocale('zh')"
            >
              <span class="lang-code">中</span>
              <span class="lang-name">中文</span>
              <Check v-if="currentLocale === 'zh'" :size="12" class="check" />
            </button>
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Settings, Check, Sun, Moon, Monitor } from 'lucide-vue-next'
import { useTheme } from '@/composables/useTheme'
import { useAccentColor, ACCENT_OPTIONS } from '@/composables/useAccentColor'
import { useForgeMode } from '@/composables/useForgeMode'
import { setLocale, getLocale } from '@/i18n'
import type { SupportedLocale } from '@/i18n'

const { t } = useI18n()
const { mode: themeMode, setMode } = useTheme()
const { current: accentCurrent, setAccent, options: accentOptions } = useAccentColor()
const { mode: forgeMode } = useForgeMode()

const isOpen = ref(false)
const menuRef = ref<HTMLDivElement>()
const currentLocale = ref<SupportedLocale>(getLocale())

function setForgeMode(val: 'gsd' | 'check') {
  forgeMode.value = val
}

function changeLocale(locale: SupportedLocale) {
  setLocale(locale)
  currentLocale.value = locale
}

const themeOptions = computed(() => [
  { value: 'light' as const, label: t('settings.themeLight'), icon: Sun },
  { value: 'dark' as const, label: t('settings.themeDark'), icon: Moon },
  { value: 'auto' as const, label: t('settings.themeSystem'), icon: Monitor },
])

function onDocClick(e: MouseEvent) {
  if (isOpen.value && menuRef.value && !menuRef.value.contains(e.target as Node)) {
    isOpen.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', onDocClick)
})
onUnmounted(() => {
  document.removeEventListener('click', onDocClick)
})
</script>

<style scoped>
.settings-menu-wrapper {
  position: relative;
}

.settings-trigger {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  background: transparent;
  border: none;
  border-radius: 6px;
  color: var(--af-muted);
  cursor: pointer;
  transition: all 0.15s;
}

.settings-trigger:hover,
.settings-trigger.open {
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-fg);
}

.settings-panel {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  min-width: 220px;
  background: var(--af-card);
  border: 1px solid var(--af-border);
  border-radius: 8px;
  padding: 0.5rem;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.08);
  z-index: 100;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.settings-section {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.settings-section-title {
  font-size: 0.73rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--af-muted);
  padding: 0 0.1rem;
}

.settings-section + .settings-section {
  border-top: 1px solid var(--af-border);
  padding-top: 0.5rem;
}

/* ─── Mode Toggle ───────────────────────────────────────────────────────── */

.mode-toggle {
  display: flex;
  align-items: center;
  gap: 1px;
  background: hsl(var(--muted-foreground) / 0.08);
  border-radius: 5px;
  padding: 2px;
}

.mode-btn {
  flex: 1;
  font-size: 0.68rem;
  font-weight: 600;
  padding: 0.25rem 0.5rem;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--af-muted);
  cursor: pointer;
  transition: all 0.15s;
  text-transform: uppercase;
  letter-spacing: 0.02em;
}

.mode-btn.active {
  background: var(--af-card);
  color: var(--af-primary);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
}

/* ─── Accent Swatches ──────────────────────────────────────────────────── */

.accent-swatches {
  display: flex;
  gap: 0.4rem;
  flex-wrap: wrap;
}

.accent-swatch {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  transition: transform 0.1s, box-shadow 0.15s;
  padding: 0;
}

.accent-swatch:hover {
  transform: scale(1.1);
}

.accent-swatch.active {
  box-shadow: 0 0 0 2px var(--af-bg), 0 0 0 4px var(--af-primary);
}

/* ─── Theme Options ────────────────────────────────────────────────────── */

.theme-options {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
}

.theme-option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.3rem 0.5rem;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--af-fg);
  font-size: 0.85rem;
  cursor: pointer;
  transition: all 0.1s;
  text-align: left;
}

.theme-option:hover {
  background: hsl(var(--muted-foreground) / 0.06);
}

.theme-option.active {
  color: var(--af-primary);
  font-weight: 500;
}

.theme-option .check {
  margin-left: auto;
  color: var(--af-primary);
}

/* ─── Language Options ─────────────────────────────────────────────────── */

.language-options {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
}

.language-option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.3rem 0.5rem;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--af-fg);
  font-size: 0.85rem;
  cursor: pointer;
  transition: all 0.1s;
  text-align: left;
}

.language-option:hover {
  background: hsl(var(--muted-foreground) / 0.06);
}

.language-option.active {
  color: var(--af-primary);
  font-weight: 500;
}

.lang-code {
  font-size: 0.75rem;
  font-weight: 600;
  width: 1.5rem;
  text-align: center;
  color: var(--af-muted);
}

.language-option.active .lang-code {
  color: var(--af-primary);
}

.lang-name {
  flex: 1;
}

.language-option .check {
  margin-left: auto;
  color: var(--af-primary);
}

/* ─── Transition ───────────────────────────────────────────────────────── */

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
  transform: translateY(4px);
}
</style>
