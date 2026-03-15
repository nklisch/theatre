<script setup lang="ts">
interface FlowStep {
  label: string
  subtitle?: string
  highlight?: 'brand' | 'amber' | 'muted'
}

interface FlowConnector {
  label?: string
  style?: 'solid' | 'dashed'
}

defineProps<{
  steps: FlowStep[]
  connectors?: FlowConnector[]
  compact?: boolean
}>()
</script>

<template>
  <div class="diagram" :class="{ compact }">
    <template v-for="(step, i) in steps" :key="i">
      <div class="flow-step" :class="step.highlight">
        <span class="flow-label">{{ step.label }}</span>
        <span class="flow-subtitle" v-if="step.subtitle">{{ step.subtitle }}</span>
      </div>
      <div class="flow-connector" v-if="i < steps.length - 1">
        <div class="flow-arrow" :class="connectors?.[i]?.style ?? 'solid'"></div>
        <span class="flow-connector-label" v-if="connectors?.[i]?.label">{{ connectors[i].label }}</span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.flow-step {
  background: var(--diagram-box-bg);
  border: 1px solid var(--diagram-box-border);
  border-radius: var(--diagram-box-radius);
  padding: 1rem 1.5rem;
  text-align: center;
  max-width: 360px;
  margin: 0 auto;
}
.flow-step.brand { border-left: 3px solid var(--diagram-accent); }
.flow-step.amber { border-left: 3px solid var(--diagram-accent-alt); }
.flow-step.muted { opacity: 0.7; }

.flow-label {
  font-family: var(--diagram-label-font);
  font-size: 0.9rem;
  color: var(--diagram-title);
  font-weight: 600;
}
.flow-subtitle {
  display: block;
  font-size: 0.8rem;
  color: var(--diagram-label);
  margin-top: 0.25rem;
}

.flow-connector {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 0.5rem 0;
}
.flow-arrow {
  width: 2px;
  height: 24px;
  background: var(--diagram-arrow);
  position: relative;
}
.flow-arrow.dashed {
  background: none;
  border-left: 2px dashed var(--diagram-arrow);
}
.flow-arrow::after {
  content: '';
  position: absolute;
  bottom: -6px;
  left: -4px;
  border-left: 5px solid transparent;
  border-right: 5px solid transparent;
  border-top: 6px solid var(--diagram-arrow);
}
.flow-connector-label {
  font-family: var(--diagram-label-font);
  font-size: 0.75rem;
  color: var(--diagram-label);
  margin-top: 0.25rem;
}

/* compact mode */
.compact .flow-step {
  padding: 0.5rem 1rem;
  max-width: 280px;
}
.compact .flow-label { font-size: 0.8rem; }
.compact .flow-arrow { height: 16px; }
</style>
