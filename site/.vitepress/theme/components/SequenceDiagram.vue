<script setup lang="ts">
interface Actor {
  label: string
  highlight?: 'brand' | 'amber'
}

interface Message {
  from: 'left' | 'right'
  to: 'left' | 'right'
  label: string
  body?: string
  style?: 'solid' | 'dashed'
  note?: string
}

defineProps<{
  left: Actor
  right: Actor
  messages: Message[]
}>()
</script>

<template>
  <div class="diagram">
    <div class="seq-header">
      <div class="seq-actor" :class="left.highlight">{{ left.label }}</div>
      <div class="seq-spacer"></div>
      <div class="seq-actor" :class="right.highlight">{{ right.label }}</div>
    </div>
    <div class="seq-body">
      <div class="seq-row" v-for="(msg, i) in messages" :key="i">
        <div class="seq-lifeline left"></div>
        <div
          class="seq-message"
          :class="[msg.from === 'left' ? 'ltr' : 'rtl', msg.style ?? 'solid']"
        >
          <span class="seq-msg-label">{{ msg.label }}</span>
          <div class="seq-arrow"></div>
          <pre class="seq-msg-body" v-if="msg.body">{{ msg.body }}</pre>
          <p class="seq-note" v-if="msg.note">{{ msg.note }}</p>
        </div>
        <div class="seq-lifeline right"></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.seq-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0;
  min-width: 400px;
}
.seq-actor {
  background: var(--diagram-box-bg);
  border: 1px solid var(--diagram-box-border);
  border-radius: var(--diagram-box-radius);
  padding: 0.5rem 1.5rem;
  font-family: var(--diagram-label-font);
  font-weight: 600;
  color: var(--diagram-title);
  font-size: 0.85rem;
  flex-shrink: 0;
}
.seq-actor.brand { border-bottom: 2px solid var(--diagram-accent); }
.seq-actor.amber { border-bottom: 2px solid var(--diagram-accent-alt); }
.seq-spacer { flex: 1; }

.seq-body {
  position: relative;
  min-width: 400px;
}
.seq-row {
  display: flex;
  align-items: stretch;
  position: relative;
}

.seq-lifeline {
  width: 1px;
  background: var(--diagram-arrow);
  opacity: 0.4;
  flex-shrink: 0;
  align-self: stretch;
}
/* align lifelines to center of actor boxes */
.seq-lifeline.left {
  margin-left: calc(2.5rem - 0.5px);
}
.seq-lifeline.right {
  margin-right: calc(2.5rem - 0.5px);
}

.seq-message {
  flex: 1;
  padding: 0.75rem 0.5rem;
  text-align: center;
}
.seq-msg-label {
  display: block;
  font-family: var(--diagram-label-font);
  font-size: 0.8rem;
  color: var(--diagram-label);
  margin-bottom: 0.35rem;
}
.seq-arrow {
  height: 2px;
  background: var(--diagram-arrow);
  margin: 0;
  position: relative;
}
.seq-message.dashed .seq-arrow {
  background: none;
  border-top: 2px dashed var(--diagram-arrow);
}

/* right-pointing arrowhead */
.seq-message.ltr .seq-arrow::after {
  content: '';
  position: absolute;
  right: -1px;
  top: -4px;
  border-top: 5px solid transparent;
  border-bottom: 5px solid transparent;
  border-left: 6px solid var(--diagram-arrow);
}
/* left-pointing arrowhead */
.seq-message.rtl .seq-arrow::after {
  content: '';
  position: absolute;
  left: -1px;
  top: -4px;
  border-top: 5px solid transparent;
  border-bottom: 5px solid transparent;
  border-right: 6px solid var(--diagram-arrow);
}

.seq-msg-body {
  font-family: var(--diagram-label-font);
  font-size: 0.72rem;
  color: var(--vp-c-text-3);
  margin: 0.35rem 0 0;
  white-space: pre;
  background: none;
  border: none;
  padding: 0;
  text-align: left;
  display: inline-block;
}
.seq-note {
  font-size: 0.75rem;
  color: var(--diagram-label);
  font-style: italic;
  margin: 0.25rem 0 0;
}
</style>
