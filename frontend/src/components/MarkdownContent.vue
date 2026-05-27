<template>
  <div ref="containerRef" class="markdown-content">
    <MarkdownRender :content="content" :final="true" :mermaid-props="{ isStrict: false }" />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import { MarkdownRender } from 'markstream-vue'
import mermaid from 'mermaid'

mermaid.initialize({ startOnLoad: false, securityLevel: 'loose' })

const props = defineProps<{
  content: string
}>()

const emit = defineEmits<{
  linkClick: [id: string]
}>()

const containerRef = ref<HTMLElement | null>(null)

// ID reference regex: G1, G1.1, A1, D1, P1, S1.1, V1, T1, R1, Relay-G1, AgentConfig-D2
const ID_RE = /\b((?:[A-Za-z]+-)?[GADPSVXTIR]\d+(?:\.\d+)?)\b/g

function processLinks() {
  const container = containerRef.value
  if (!container) return

  // Walk all text nodes inside the rendered markdown
  const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT, null)
  const textNodes: Text[] = []
  let node: Node | null
  while ((node = walker.nextNode())) {
    // Only process text nodes that are direct children of elements (not inside <a> or <code>)
    const parent = node.parentElement
    if (parent && (parent.tagName === 'A' || parent.tagName === 'CODE' || parent.tagName === 'PRE')) continue
    textNodes.push(node as Text)
  }

  for (const textNode of textNodes) {
    const text = textNode.textContent || ''
    if (!ID_RE.test(text)) continue
    ID_RE.lastIndex = 0

    const frag = document.createDocumentFragment()
    let lastIndex = 0
    let match: RegExpExecArray | null

    while ((match = ID_RE.exec(text))) {
      // Append text before match
      if (match.index > lastIndex) {
        frag.appendChild(document.createTextNode(text.slice(lastIndex, match.index)))
      }
      // Create clickable link
      const span = document.createElement('span')
      span.className = 'spec-link'
      span.textContent = match[1]
      span.addEventListener('click', (e) => {
        e.stopPropagation()
        emit('linkClick', match![1])
      })
      frag.appendChild(span)
      lastIndex = match.index + match[0].length
    }

    if (lastIndex < text.length) {
      frag.appendChild(document.createTextNode(text.slice(lastIndex)))
    }

    if (frag.childNodes.length > 0) {
      textNode.parentNode?.replaceChild(frag, textNode)
    }
  }
}

async function renderMermaid() {
  const container = containerRef.value
  if (!container) return

  // Case 1: markstream-vue rendered mermaid as plain <pre><code>
  const codeBlocks = Array.from(container.querySelectorAll('pre code.language-mermaid'))
  for (const code of codeBlocks) {
    const pre = code.parentElement as HTMLPreElement
    if (pre.dataset.mermaidRendered === 'true') continue

    const graphDefinition = code.textContent || ''
    try {
      const id = 'mermaid-' + Math.random().toString(36).slice(2)
      const { svg } = await mermaid.render(id, graphDefinition)
      const wrapper = document.createElement('div')
      wrapper.className = 'mermaid-diagram'
      wrapper.innerHTML = svg
      pre.replaceWith(wrapper)
    } catch (e) {
      // Leave as plain code block if parsing fails
    }
  }

  // Case 2: markstream-vue MermaidBlockNode rendered a shell but failed to produce SVG
  const mermaidBlocks = Array.from(container.querySelectorAll('.mermaid-block'))
  for (const block of mermaidBlocks) {
    if (block.querySelector('svg')) continue // already rendered

    const codeEl = block.querySelector('code')
    const graphDefinition = codeEl?.textContent || block.textContent || ''
    try {
      const id = 'mermaid-' + Math.random().toString(36).slice(2)
      const { svg } = await mermaid.render(id, graphDefinition)
      const wrapper = document.createElement('div')
      wrapper.className = 'mermaid-diagram'
      wrapper.innerHTML = svg
      block.replaceWith(wrapper)
    } catch (e) {
      // Leave as-is if parsing fails
    }
  }
}

watch(() => props.content, () => {
  nextTick(() => {
    processLinks()
    renderMermaid()
  })
}, { immediate: true })
</script>

<style scoped>
.markdown-content :deep(.spec-link) {
  display: inline;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.85em;
  font-weight: 600;
  color: hsl(var(--primary));
  background: hsl(var(--primary) / 0.08);
  padding: 0.05em 0.35em;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.12s;
}
.markdown-content :deep(.spec-link:hover) {
  background: hsl(var(--primary) / 0.18);
}
</style>
