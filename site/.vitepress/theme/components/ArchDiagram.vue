<script setup lang="ts">
/**
 * Renders the Theatre architecture flow diagram.
 * Props control which path to highlight (stage, director, or both).
 */
defineProps<{
  highlight?: 'stage' | 'director' | 'both'
}>()
</script>

<template>
  <div class="arch-diagram">
    <pre :class="['arch-pre', highlight]">
┌─────────────────────────────────────────────────────┐
│                    AI Agent                         │
│           (Claude Code, Cursor, etc.)               │
└──────┬──────────────────────┬───────────────────────┘
       │                      │
    Stage (MCP)          Director (MCP)
  <em>observe &amp; interact</em>    <em>build the game</em>
       │                      │
  ┌────▼────────┐      ┌──────▼──────┐
  │   stage     │      │  director   │
  │   server    │      │   server    │
  └────┬────────┘      └──────┬──────┘
       │ TCP :9077            │ TCP :6551/:6550
  ┌────▼────────┐      ┌──────▼──────┐
  │   Godot     │      │   Godot     │
  │ GDExtension │      │ GDScript    │
  │  (running)  │      │  (editor)   │
  └─────────────┘      └─────────────┘</pre>
  </div>
</template>

<style scoped>
.arch-pre {
  margin: 0;
  white-space: pre;
}
.arch-pre em {
  font-style: normal;
  color: var(--vp-c-text-3);
}
.arch-pre.stage em:first-of-type,
.arch-pre.both em {
  color: var(--vp-c-brand-1);
}
.arch-pre.director em:last-of-type,
.arch-pre.both em {
  color: var(--theatre-amber);
}
</style>
