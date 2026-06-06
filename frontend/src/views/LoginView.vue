<script setup lang="ts">
import { ref } from 'vue'
import { useAuth } from '@/composables/useAuth'
import { useI18n } from 'vue-i18n'
import { Flame } from 'lucide-vue-next'

const { t } = useI18n()
const emit = defineEmits<{ 'auth-success': [] }>()

const { login, register, loading, error, isAuthenticated } = useAuth()

const username = ref('')
const password = ref('')
const isRegisterMode = ref(false)

async function handleSubmit() {
  const success = isRegisterMode.value
    ? await register(username.value, password.value)
    : await login(username.value, password.value)
  if (success) {
    emit('auth-success')
  }
}

function toggleMode() {
  isRegisterMode.value = !isRegisterMode.value
  error.value = null
}
</script>

<template>
  <div class="login-view">
    <div class="login-card">
      <div class="login-brand">
        <Flame :size="32" />
        <h1>AutoForge</h1>
      </div>

      <form @submit.prevent="handleSubmit" class="login-form">
        <div class="form-group">
          <label for="username">{{ t('auth.username') }}</label>
          <input
            id="username"
            v-model="username"
            type="text"
            autocomplete="username"
            required
            :disabled="loading"
            :placeholder="t('auth.usernamePlaceholder')"
          />
        </div>

        <div class="form-group">
          <label for="password">{{ t('auth.password') }}</label>
          <input
            id="password"
            v-model="password"
            type="password"
            autocomplete="current-password"
            required
            :disabled="loading"
            :placeholder="t('auth.passwordPlaceholder')"
          />
        </div>

        <div v-if="error" class="login-error" role="alert">
          {{ error }}
        </div>

        <button type="submit" class="login-button" :disabled="loading">
          {{ loading
            ? t('auth.loading')
            : isRegisterMode
              ? t('auth.register')
              : t('auth.login')
          }}
        </button>
      </form>

      <div class="login-footer">
        <button class="toggle-mode" @click="toggleMode">
          {{ isRegisterMode
            ? t('auth.hasAccount')
            : t('auth.noAccount')
          }}
        </button>
      </div>

      <div class="login-hint">
        {{ t('auth.defaultHint') }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.login-view {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  background: hsl(var(--af-bg));
}

.login-card {
  width: 100%;
  max-width: 380px;
  padding: 2rem;
  background: hsl(var(--card));
  border: 1px solid var(--af-border);
  border-radius: 12px;
  box-shadow: 0 4px 24px rgba(0,0,0,0.1);
}

.login-brand {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 1.5rem;
  color: var(--af-primary);
}

.login-brand h1 {
  font-size: 1.5rem;
  font-weight: 700;
}

.login-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.form-group label {
  font-size: 0.85rem;
  font-weight: 500;
  color: var(--af-muted);
}

.form-group input {
  padding: 0.6rem 0.75rem;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  background: hsl(var(--af-bg));
  color: var(--af-fg);
  font-size: 0.95rem;
}

.form-group input:focus {
  outline: none;
  border-color: var(--af-primary);
  box-shadow: 0 0 0 2px hsl(var(--primary) / 0.15);
}

.login-error {
  padding: 0.5rem 0.75rem;
  border-radius: 6px;
  background: hsl(var(--destructive) / 0.1);
  color: hsl(var(--destructive));
  font-size: 0.85rem;
}

.login-button {
  padding: 0.65rem 1rem;
  border: none;
  border-radius: 6px;
  background: var(--af-primary);
  color: #fff;
  font-size: 0.95rem;
  font-weight: 600;
  cursor: pointer;
  transition: opacity 0.15s;
}

.login-button:hover { opacity: 0.9; }
.login-button:disabled { opacity: 0.5; cursor: not-allowed; }

.login-footer {
  margin-top: 1rem;
  text-align: center;
}

.toggle-mode {
  background: none;
  border: none;
  color: var(--af-primary);
  font-size: 0.85rem;
  cursor: pointer;
  text-decoration: underline;
}

.toggle-mode:hover { opacity: 0.8; }

.login-hint {
  margin-top: 1rem;
  text-align: center;
  font-size: 0.78rem;
  color: var(--af-muted);
}
</style>
