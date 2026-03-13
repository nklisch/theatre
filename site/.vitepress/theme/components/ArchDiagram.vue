<script setup lang="ts">
/**
 * Renders the Theatre architecture flow diagram.
 * Props control which path to highlight (spectator, director, or both).
 */
defineProps<{
  highlight?: 'spectator' | 'director' | 'both'
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
  Spectator (MCP)        Director (MCP)
  <em>observe the game</em>      <em>build the game</em>
       │                      │
  ┌────▼────────┐      ┌──────▼──────┐
  │  spectator  │      │  director   │
  │  -server    │      │   server    │
  └────┬────────┘      └──────┬──────┘
       │ TCP :9077            │ TCP :6550/:6551
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
.arch-pre.spectator em:first-of-type,
.arch-pre.both em {
  color: var(--vp-c-brand-1);
}
.arch-pre.director em:last-of-type,
.arch-pre.both em {
  color: var(--theatre-amber);
}
</style>
