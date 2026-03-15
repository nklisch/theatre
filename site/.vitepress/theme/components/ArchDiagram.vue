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
  <div class="diagram arch-layout">
    <!-- Top: AI Agent -->
    <div class="arch-box arch-top">
      <span class="arch-label">AI Agent</span>
      <span class="arch-subtitle">(Claude Code, Cursor, etc.)</span>
    </div>

    <!-- Split connector row -->
    <div class="arch-split-row">
      <div class="arch-branch">
        <div class="arch-connector-line"></div>
        <div
          class="arch-connector-label"
          :class="{ active: highlight === 'stage' || highlight === 'both', brand: true }"
        >Stage (MCP)</div>
        <div class="arch-connector-sublabel">observe &amp; interact</div>
        <div class="arch-connector-line"></div>
      </div>
      <div class="arch-branch">
        <div class="arch-connector-line"></div>
        <div
          class="arch-connector-label"
          :class="{ active: highlight === 'director' || highlight === 'both', amber: true }"
        >Director (MCP)</div>
        <div class="arch-connector-sublabel">build the game</div>
        <div class="arch-connector-line"></div>
      </div>
    </div>

    <!-- Server boxes -->
    <div class="arch-row">
      <div class="arch-box" :class="{ 'highlight-brand': highlight === 'stage' || highlight === 'both' }">
        <span class="arch-label">stage server</span>
      </div>
      <div class="arch-box" :class="{ 'highlight-amber': highlight === 'director' || highlight === 'both' }">
        <span class="arch-label">director server</span>
      </div>
    </div>

    <!-- TCP labels -->
    <div class="arch-split-row arch-tcp-row">
      <div class="arch-branch">
        <div class="arch-connector-line"></div>
        <div class="arch-tcp-label">TCP :9077</div>
        <div class="arch-connector-line"></div>
      </div>
      <div class="arch-branch">
        <div class="arch-connector-line"></div>
        <div class="arch-tcp-label">TCP :6551/:6550</div>
        <div class="arch-connector-line"></div>
      </div>
    </div>

    <!-- Godot boxes -->
    <div class="arch-row">
      <div class="arch-box arch-godot">
        <span class="arch-label">Godot GDExtension</span>
        <span class="arch-subtitle">(running)</span>
      </div>
      <div class="arch-box arch-godot">
        <span class="arch-label">Godot GDScript</span>
        <span class="arch-subtitle">(editor)</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.arch-layout {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0;
  min-width: 420px;
}

.arch-top {
  width: 100%;
  max-width: 480px;
}

.arch-box {
  background: var(--diagram-box-bg);
  border: 1px solid var(--diagram-box-border);
  border-radius: var(--diagram-box-radius);
  padding: 0.75rem 1.5rem;
  text-align: center;
  flex: 1;
}
.arch-box.highlight-brand { border-left: 3px solid var(--diagram-accent); }
.arch-box.highlight-amber { border-left: 3px solid var(--diagram-accent-alt); }

.arch-label {
  display: block;
  font-family: var(--diagram-label-font);
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--diagram-title);
}
.arch-subtitle {
  display: block;
  font-size: 0.75rem;
  color: var(--diagram-label);
  margin-top: 0.2rem;
}

.arch-split-row {
  display: flex;
  width: 100%;
  max-width: 480px;
  gap: 2rem;
  justify-content: center;
}
.arch-branch {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
}
.arch-connector-line {
  width: 1px;
  height: 16px;
  background: var(--diagram-arrow);
  opacity: 0.5;
}
.arch-connector-label {
  font-family: var(--diagram-label-font);
  font-size: 0.78rem;
  font-weight: 600;
  color: var(--diagram-label);
  padding: 0.1rem 0.5rem;
}
.arch-connector-label.brand.active { color: var(--diagram-accent); }
.arch-connector-label.amber.active { color: var(--diagram-accent-alt); }
.arch-connector-sublabel {
  font-size: 0.72rem;
  color: var(--diagram-label);
  opacity: 0.75;
}

.arch-row {
  display: flex;
  width: 100%;
  max-width: 480px;
  gap: 2rem;
}
.arch-row .arch-box {
  flex: 1;
}

.arch-tcp-row .arch-branch {
  gap: 0;
}
.arch-tcp-label {
  font-family: var(--diagram-label-font);
  font-size: 0.72rem;
  color: var(--diagram-label);
  padding: 0.1rem 0;
}

.arch-godot {
  background: var(--diagram-box-bg);
}
</style>
