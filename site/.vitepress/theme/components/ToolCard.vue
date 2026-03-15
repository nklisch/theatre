<script setup lang="ts">
import { withBase } from 'vitepress'
defineProps<{
  title: string
  icon: string        // emoji or SVG
  description: string
  tool: string        // MCP tool name
  tokens: string      // typical token cost range
  link: string        // docs link
}>()
</script>

<template>
  <a :href="withBase(link)" class="tool-card">
    <div class="tool-card-icon">
      <img v-if="icon.startsWith('/')" :src="withBase(icon)" :alt="title + ' icon'" class="tool-card-svg" />
      <span v-else>{{ icon }}</span>
    </div>
    <h3 class="tool-card-title">{{ title }}</h3>
    <p class="tool-card-desc">{{ description }}</p>
    <div class="tool-card-footer">
      <code class="tool-card-name">{{ tool }}</code>
      <span class="tool-card-tokens">~{{ tokens }} tokens</span>
    </div>
  </a>
</template>

<style scoped>
.tool-card {
  display: block;
  padding: 1.5rem;
  border: 1px solid var(--vp-c-border);
  border-radius: 12px;
  background: var(--vp-c-bg-soft);
  text-decoration: none;
  color: inherit;
  transition: border-color 0.2s, box-shadow 0.2s;
}
.tool-card:hover {
  border-color: var(--vp-c-brand-1);
  box-shadow: 0 0 20px rgba(71, 140, 191, 0.1);
}
.tool-card-icon { font-size: 2rem; margin-bottom: 0.75rem; }
.tool-card-svg { width: 48px; height: 48px; }
.tool-card-title {
  font-size: 1.1rem;
  font-weight: 600;
  margin: 0 0 0.5rem;
  color: var(--vp-c-text-1);
}
.tool-card-desc {
  font-size: 0.9rem;
  color: var(--vp-c-text-2);
  line-height: 1.5;
  margin: 0 0 1rem;
}
.tool-card-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.tool-card-name {
  font-size: 0.8rem;
  color: var(--vp-c-brand-1);
}
.tool-card-tokens {
  font-size: 0.75rem;
  color: var(--vp-c-text-3);
}
</style>
