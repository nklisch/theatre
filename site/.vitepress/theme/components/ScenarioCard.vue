<script setup lang="ts">
import { withBase } from 'vitepress'
defineProps<{
  title: string
  problem: string    // 1-line problem statement
  icon: string
  link: string
}>()
</script>

<template>
  <a :href="withBase(link)" class="scenario-card">
    <div class="scenario-icon">{{ icon }}</div>
    <h3 class="scenario-title">{{ title }}</h3>
    <p class="scenario-problem">{{ problem }}</p>
    <span class="scenario-cta">See how to debug this →</span>
  </a>
</template>

<style scoped>
.scenario-card {
  display: block;
  padding: 1.5rem;
  border: 1px solid var(--vp-c-border);
  border-radius: 12px;
  background: var(--vp-c-bg-soft);
  text-decoration: none;
  color: inherit;
  transition: border-color 0.2s, transform 0.15s;
}
.scenario-card:hover {
  border-color: var(--theatre-amber);
  transform: translateY(-2px);
}
.scenario-icon { font-size: 2rem; margin-bottom: 0.75rem; }
.scenario-title {
  font-size: 1.05rem;
  font-weight: 600;
  margin: 0 0 0.5rem;
  color: var(--vp-c-text-1);
}
.scenario-problem {
  font-size: 0.85rem;
  color: var(--vp-c-text-2);
  line-height: 1.5;
  margin: 0 0 1rem;
}
.scenario-cta {
  font-size: 0.8rem;
  color: var(--theatre-amber);
  font-weight: 500;
}
</style>
