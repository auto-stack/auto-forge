<template>
  <div class="questionnaire-card">
    <div class="q-header">
      <HelpCircle :size="16" />
      <span>Quick Questions</span>
    </div>
    <div v-for="(q, idx) in questions" :key="q.id" class="q-item">
      <div class="q-text">Q{{ idx + 1 }}. {{ q.text }}</div>
      <div v-if="q.type === 'single'" class="q-options">
        <label
          v-for="opt in q.options"
          :key="opt"
          class="q-option"
          :class="{ checked: answers[q.id] === opt }"
        >
          <input type="radio" :name="q.id" :value="opt" v-model="answers[q.id]" />
          <span class="q-check" />
          <span class="q-label">{{ opt }}</span>
        </label>
        <div v-if="q.otherLabel" class="q-other-row">
          <span class="q-other-label">{{ q.otherLabel }}</span>
          <input
            v-model="answers[q.id + '__other']"
            type="text"
            class="q-other-input"
            :placeholder="q.otherPlaceholder || 'Type here...'"
          />
        </div>
      </div>
      <div v-else-if="q.type === 'multiple'" class="q-options">
        <label
          v-for="opt in q.options"
          :key="opt"
          class="q-option"
          :class="{ checked: (answers[q.id] as string[] || []).includes(opt) }"
        >
          <input type="checkbox" :value="opt" v-model="answers[q.id]" />
          <span class="q-check square" />
          <span class="q-label">{{ opt }}</span>
        </label>
        <div v-if="q.otherLabel" class="q-other-row">
          <span class="q-other-label">{{ q.otherLabel }}</span>
          <input
            v-model="answers[q.id + '__other']"
            type="text"
            class="q-other-input"
            :placeholder="q.otherPlaceholder || 'Type here...'"
          />
        </div>
      </div>
      <div v-else-if="q.type === 'text'" class="q-text-input">
        <input
          v-model="answers[q.id]"
          type="text"
          :placeholder="q.placeholder || 'Type your answer...'"
          @keydown.enter.prevent="submit"
        />
      </div>
    </div>
    <button class="q-submit" :disabled="!canSubmit" @click="submit">
      <Send :size="13" />
      Submit Answers
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { HelpCircle, Send } from 'lucide-vue-next'

export interface Question {
  id: string
  text: string
  type: 'single' | 'multiple' | 'text'
  options?: string[]
  placeholder?: string
  optional?: boolean
  otherLabel?: string
  otherPlaceholder?: string
}

const props = defineProps<{
  questions: Question[]
}>()

const emit = defineEmits<{
  submit: [answers: Record<string, string | string[]>]
}>()

const answers = ref<Record<string, string | string[]>>({})

// Initialize defaults
for (const q of props.questions) {
  if (q.type === 'multiple' && !answers.value[q.id]) {
    answers.value[q.id] = []
  }
}

const canSubmit = computed(() => {
  for (const q of props.questions) {
    if (q.optional) continue
    const ans = answers.value[q.id]
    if (q.type === 'multiple') {
      if (!(ans as string[]).length) return false
    } else {
      if (!ans || (ans as string).trim() === '') return false
    }
  }
  return true
})

function submit() {
  emit('submit', { ...answers.value })
}
</script>

<style scoped>
.questionnaire-card {
  background: hsl(var(--primary) / 0.04);
  border: 1px solid hsl(var(--primary) / 0.15);
  border-radius: 10px;
  padding: 0.75rem 1rem;
  margin-top: 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.q-header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--af-primary);
}

.q-item {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.q-text {
  font-size: 0.88rem;
  font-weight: 500;
  color: var(--af-fg);
  line-height: 1.4;
}

.q-options {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.q-option {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.35rem 0.5rem;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.1s;
  font-size: 0.85rem;
  color: var(--af-fg);
}

.q-option:hover {
  background: hsl(var(--primary) / 0.06);
}

.q-option.checked {
  background: hsl(var(--primary) / 0.08);
}

.q-option input {
  display: none;
}

.q-check {
  width: 14px;
  height: 14px;
  border: 2px solid var(--af-border);
  border-radius: 50%;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
}

.q-check.square {
  border-radius: 4px;
}

.q-option.checked .q-check {
  border-color: var(--af-primary);
  background: var(--af-primary);
}

.q-option.checked .q-check::after {
  content: '';
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #fff;
}

.q-option.checked .q-check.square::after {
  width: 6px;
  height: 3px;
  border-radius: 0;
  background: transparent;
  border-left: 2px solid #fff;
  border-bottom: 2px solid #fff;
  transform: rotate(-45deg);
  margin-bottom: 1px;
}

.q-label {
  line-height: 1.3;
}

.q-text-input input {
  width: 100%;
  padding: 0.4rem 0.6rem;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  background: var(--af-bg);
  color: var(--af-fg);
  font-size: 0.85rem;
  outline: none;
  transition: border-color 0.15s;
}

.q-text-input input:focus {
  border-color: var(--af-primary);
}

.q-other-row {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.35rem 0.5rem;
  font-size: 0.85rem;
  color: var(--af-fg);
}

.q-other-label {
  flex-shrink: 0;
  color: var(--af-muted);
}

.q-other-input {
  flex: 1;
  min-width: 0;
  padding: 0.3rem 0.5rem;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  background: var(--af-bg);
  color: var(--af-fg);
  font-size: 0.85rem;
  outline: none;
  transition: border-color 0.15s;
}

.q-other-input:focus {
  border-color: var(--af-primary);
}

.q-submit {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.3rem;
  align-self: flex-start;
  padding: 0.4rem 0.8rem;
  border: none;
  border-radius: 6px;
  background: var(--af-primary);
  color: #fff;
  font-size: 0.82rem;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.15s;
}

.q-submit:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.q-submit:hover:not(:disabled) {
  opacity: 0.9;
}
</style>
