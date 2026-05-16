<template>
  <div class="tree-node">
    <div
      class="tree-row"
      :class="{ active: activePath === node.path }"
      @click="handleClick"
    >
      <span v-if="node.type === 'folder'" class="tree-chevron" @click.stop="toggle">
        <ChevronRight v-if="!expanded" :size="12" />
        <ChevronDown v-else :size="12" />
      </span>
      <span v-else class="tree-chevron-spacer" />
      <component :is="fileIcon" :size="13" class="tree-icon" />
      <span class="tree-name" :title="node.name">{{ node.name }}</span>
    </div>
    <div v-if="node.type === 'folder' && expanded" class="tree-children">
      <TreeView
        v-for="child in node.children"
        :key="child.path"
        :node="child"
        :active-path="activePath"
        @select="$emit('select', $event)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import {
  ChevronRight, ChevronDown,
  FileText, File, Image, FileCode, Folder,
} from 'lucide-vue-next'
import type { TreeNode } from '@/types/wiki'

// Required for recursive component
defineOptions({ name: 'TreeView' })

const props = defineProps<{
  node: TreeNode
  activePath: string
}>()

const emit = defineEmits<{
  select: [payload: { path: string; type: string }]
}>()

const expanded = ref(true)

function toggle() {
  expanded.value = !expanded.value
}

function handleClick() {
  emit('select', { path: props.node.path, type: props.node.type })
}

const fileIcon = computed(() => {
  if (props.node.type === 'folder') return Folder
  const ext = props.node.name.split('.').pop()?.toLowerCase()
  switch (ext) {
    case 'md':
    case 'txt':
    case 'pdf':
    case 'doc':
    case 'docx':
      return FileText
    case 'png':
    case 'jpg':
    case 'jpeg':
    case 'gif':
    case 'svg':
    case 'webp':
      return Image
    case 'json':
    case 'csv':
    case 'xml':
    case 'yaml':
    case 'yml':
      return FileCode
    default:
      return File
  }
})
</script>

<style scoped>
.tree-row {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0.3rem 0.5rem;
  cursor: pointer;
  border-left: 3px solid transparent;
  transition: background 0.1s;
}

.tree-row:hover {
  background: hsl(var(--muted-foreground) / 0.05);
}

.tree-row.active {
  background: hsl(var(--primary) / 0.06);
  border-left-color: hsl(var(--primary));
}

.tree-chevron {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  flex-shrink: 0;
  color: var(--af-muted);
}

.tree-chevron-spacer {
  width: 14px;
  flex-shrink: 0;
}

.tree-icon {
  flex-shrink: 0;
  color: var(--af-muted);
}

.tree-row.active .tree-icon {
  color: hsl(var(--primary));
}

.tree-name {
  font-size: 0.86rem;
  color: var(--af-fg);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tree-children {
  padding-left: 0.75rem;
}
</style>
