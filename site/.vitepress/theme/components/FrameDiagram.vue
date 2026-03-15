<script setup lang="ts">
interface FrameSection {
  label: string
  detail: string
  flex?: number
  highlight?: 'brand' | 'amber' | 'muted'
}

defineProps<{
  sections: FrameSection[]
  title?: string
}>()
</script>

<template>
  <div class="diagram">
    <div class="frame-title" v-if="title">{{ title }}</div>
    <div class="frame-row">
      <div
        v-for="(s, i) in sections"
        :key="i"
        class="frame-section"
        :class="[s.highlight, { first: i === 0, last: i === sections.length - 1 }]"
        :style="{ flex: s.flex ?? 1 }"
      >
        <span class="frame-label">{{ s.label }}</span>
        <span class="frame-detail">{{ s.detail }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.frame-title {
  text-align: center;
  font-family: var(--diagram-label-font);
  font-size: 0.85rem;
  color: var(--diagram-label);
  margin-bottom: 1rem;
}
.frame-row {
  display: flex;
  max-width: 480px;
  margin: 0 auto;
}
.frame-section {
  border: 1px solid var(--diagram-box-border);
  padding: 0.75rem 1rem;
  text-align: center;
  background: var(--diagram-box-bg);
}
.frame-section + .frame-section {
  border-left: none;
}
.frame-section.first {
  border-radius: var(--diagram-box-radius) 0 0 var(--diagram-box-radius);
}
.frame-section.last {
  border-radius: 0 var(--diagram-box-radius) var(--diagram-box-radius) 0;
}
.frame-section.brand { border-top: 2px solid var(--diagram-accent); }
.frame-section.amber { border-top: 2px solid var(--diagram-accent-alt); }
.frame-label {
  display: block;
  font-family: var(--diagram-label-font);
  font-weight: 600;
  font-size: 0.85rem;
  color: var(--diagram-title);
}
.frame-detail {
  display: block;
  font-size: 0.75rem;
  color: var(--diagram-label);
  margin-top: 0.25rem;
}
</style>
