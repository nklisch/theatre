# Design: Site Diagram Components

## Overview

Replace all ASCII art diagrams in `site/` (public VitePress docs) with styled Vue components. The goal is a small set of reusable, prop-driven components that cover all current diagram types — not one component per diagram.

**Scope**: 7 ASCII diagrams across 3 site pages, plus the existing `ArchDiagram.vue` on the homepage. Internal docs (`docs/`, `.agents/`) are out of scope — ASCII stays there.

**Non-goals**: General-purpose diagramming library. These components serve Theatre's specific documentation needs.

## Diagram Inventory (Site Only)

| # | Page | Diagram | Type | Component |
|---|------|---------|------|-----------|
| 1 | `guide/how-it-works.md` | Architecture data flow (AI → server → GDExtension → game) | Vertical flow | `FlowDiagram` |
| 2 | `architecture/tcp.md` | TCP frame layout (length + payload) | Stacked frame | `FrameDiagram` |
| 3 | `architecture/tcp.md` | Stage connection lifecycle (handshake exchange) | Sequence | `SequenceDiagram` |
| 4 | `architecture/tcp.md` | Director connection lifecycle (stateless flow) | Vertical flow | `FlowDiagram` |
| 5 | `architecture/tcp.md` | Error layering chain | Vertical flow | `FlowDiagram` |
| 6 | `architecture/tcp.md` | Persistent connection request-response | Sequence | `SequenceDiagram` |
| 7 | `architecture/crates.md` | Crate dependency graph | Dependency | `DepGraph` |
| 8 | `index.md` | Homepage architecture (Stage + Director split) | — | `ArchDiagram` (keep, restyle) |

## Shared Design Language

All diagram components share a consistent visual system via CSS custom properties in `custom.css`.

### Diagram tokens (add to `custom.css`)

```css
/* === Diagram tokens === */
:root {
  --diagram-bg: var(--vp-c-bg-alt);
  --diagram-border: var(--vp-c-border);
  --diagram-box-bg: var(--vp-c-bg-soft);
  --diagram-box-border: var(--vp-c-border);
  --diagram-box-radius: 8px;
  --diagram-arrow: var(--vp-c-text-3);
  --diagram-label: var(--vp-c-text-2);
  --diagram-label-font: var(--vp-font-family-mono);
  --diagram-title: var(--vp-c-text-1);
  --diagram-accent: var(--vp-c-brand-1);
  --diagram-accent-alt: var(--theatre-amber);
}
```

### Shared wrapper style

Every diagram component wraps its content in a `.diagram` container:

```css
.diagram {
  padding: 2rem;
  background: var(--diagram-bg);
  border-radius: 12px;
  border: 1px solid var(--diagram-border);
  margin: 2rem 0;
  overflow-x: auto;
}
```

This matches the existing `.arch-diagram` style — the ArchDiagram restyle just switches to the shared class.

---

## Implementation Units

### Unit 1: `FlowDiagram.vue`

**File**: `site/.vitepress/theme/components/FlowDiagram.vue`

A vertical stack of labeled boxes connected by arrows with optional connector labels. Covers architecture overviews, error layering chains, and simple sequential flows.

```typescript
interface FlowStep {
  label: string           // Box title (rendered in mono)
  subtitle?: string       // Secondary text below label
  highlight?: 'brand' | 'amber' | 'muted'  // Box accent color
}

interface FlowConnector {
  label?: string          // Text on the arrow between steps
  style?: 'solid' | 'dashed'
}

// Props
defineProps<{
  steps: FlowStep[]
  connectors?: FlowConnector[]  // connectors[i] is between steps[i] and steps[i+1]
  compact?: boolean             // Smaller boxes + spacing for chains like error layering
}>()
```

**Rendering approach**:
- Each step renders as a CSS box (flexbox column, centered)
- Between boxes: a vertical line (CSS `::after` pseudo-element or a `<div class="connector">`) with an SVG arrowhead and optional label
- `compact` mode reduces padding, font size, and arrow length — used for the error layering chain where steps are short labels, not full boxes
- Arrow: simple SVG triangle, colored with `--diagram-arrow`

**Markup structure**:

```html
<div class="diagram">
  <div class="flow-step" :class="step.highlight">
    <span class="flow-label">{{ step.label }}</span>
    <span class="flow-subtitle" v-if="step.subtitle">{{ step.subtitle }}</span>
  </div>
  <!-- connector -->
  <div class="flow-connector">
    <div class="flow-arrow"></div>
    <span class="flow-connector-label" v-if="connector?.label">{{ connector.label }}</span>
  </div>
  <!-- next step... -->
</div>
```

**CSS sketch**:

```css
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
.flow-arrow::after {
  /* CSS triangle arrowhead */
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
.diagram.compact .flow-step {
  padding: 0.5rem 1rem;
  max-width: 280px;
}
.diagram.compact .flow-label { font-size: 0.8rem; }
.diagram.compact .flow-arrow { height: 16px; }
```

**Usage examples**:

Architecture data flow (`how-it-works.md`):
```html
<FlowDiagram :steps="[
  { label: 'AI Agent (Claude)', subtitle: '\"Where is the player?\"' },
  { label: 'stage server (Rust)', subtitle: 'Translates MCP ↔ TCP protocol' },
  { label: 'stage GDExtension (Rust)', subtitle: 'Runs inside your Godot game' },
  { label: 'Running Godot game', subtitle: 'CharacterBody3D, Area3D, ...' },
]" :connectors="[
  { label: 'MCP (stdio)' },
  { label: 'TCP port 9077' },
  { label: 'Godot engine APIs' },
]" />
```

Error layering (`tcp.md`):
```html
<FlowDiagram compact :steps="[
  { label: 'CodecError', subtitle: 'protocol' },
  { label: 'TcpSessionError', subtitle: 'session.rs' },
  { label: 'anyhow::Error', subtitle: 'tool handlers' },
  { label: 'McpError::internal_error', subtitle: 'tool response' },
]" :connectors="[
  { label: 'wrapped by' },
  { label: 'wrapped by' },
  { label: 'converted to' },
]" />
```

Director connection lifecycle (`tcp.md`):
```html
<FlowDiagram compact :steps="[
  { label: 'Tool call' },
  { label: 'Connect to port 6551 or 6550' },
  { label: 'Send operation JSON' },
  { label: 'Read response JSON' },
  { label: 'Close connection' },
]" />
```

**Acceptance Criteria**:
- [ ] Renders vertical stack of styled boxes with arrow connectors
- [ ] Connector labels appear centered on the arrow
- [ ] `highlight` prop applies left-border accent color
- [ ] `compact` mode reduces sizing for chain-style diagrams
- [ ] Looks correct in both dark and light VitePress themes
- [ ] Responsive: scrolls horizontally on narrow viewports

---

### Unit 2: `SequenceDiagram.vue`

**File**: `site/.vitepress/theme/components/SequenceDiagram.vue`

A two-actor message exchange diagram. Covers TCP handshake lifecycle and persistent connection request-response patterns.

```typescript
interface Actor {
  label: string
  highlight?: 'brand' | 'amber'
}

interface Message {
  from: 'left' | 'right'
  to: 'left' | 'right'
  label: string
  body?: string       // Optional JSON/detail shown below the arrow
  style?: 'solid' | 'dashed'
  note?: string       // Text annotation below the message group
}

defineProps<{
  left: Actor
  right: Actor
  messages: Message[]
}>()
```

**Rendering approach**:
- Two column headers (actor boxes) at the top
- Vertical lifelines (thin dashed lines) below each actor
- Horizontal arrows between lifelines for each message
- Arrow uses SVG `<line>` + `<polygon>` arrowhead, or pure CSS
- `body` text renders in a small mono-font block below the arrow
- `note` renders as centered italic text spanning both columns

**Markup structure**:

```html
<div class="diagram">
  <div class="seq-header">
    <div class="seq-actor" :class="left.highlight">{{ left.label }}</div>
    <div class="seq-spacer"></div>
    <div class="seq-actor" :class="right.highlight">{{ right.label }}</div>
  </div>
  <div class="seq-body">
    <!-- Each message row -->
    <div class="seq-row" v-for="msg in messages">
      <div class="seq-lifeline left"></div>
      <div class="seq-message" :class="[msg.from === 'left' ? 'ltr' : 'rtl', msg.style]">
        <span class="seq-msg-label">{{ msg.label }}</span>
        <div class="seq-arrow"></div>
        <pre class="seq-msg-body" v-if="msg.body">{{ msg.body }}</pre>
      </div>
      <div class="seq-lifeline right"></div>
    </div>
  </div>
</div>
```

**CSS sketch**:

```css
.seq-header {
  display: flex;
  justify-content: space-between;
  margin-bottom: 1rem;
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
}
.seq-actor.brand { border-bottom: 2px solid var(--diagram-accent); }
.seq-actor.amber { border-bottom: 2px solid var(--diagram-accent-alt); }

.seq-body {
  position: relative;
  padding: 0 2rem;
}
.seq-row {
  display: flex;
  align-items: center;
  padding: 0.75rem 0;
  position: relative;
}

.seq-message {
  flex: 1;
  position: relative;
  text-align: center;
}
.seq-msg-label {
  font-family: var(--diagram-label-font);
  font-size: 0.8rem;
  color: var(--diagram-label);
}
.seq-arrow {
  height: 2px;
  background: var(--diagram-arrow);
  margin: 0.5rem 0;
  position: relative;
}
/* Arrowhead direction based on ltr/rtl class */
.seq-message.ltr .seq-arrow::after {
  /* right-pointing arrowhead */
  content: '';
  position: absolute;
  right: -1px;
  top: -4px;
  border-top: 5px solid transparent;
  border-bottom: 5px solid transparent;
  border-left: 6px solid var(--diagram-arrow);
}
.seq-message.rtl .seq-arrow::after {
  /* left-pointing arrowhead */
  content: '';
  position: absolute;
  left: -1px;
  top: -4px;
  border-top: 5px solid transparent;
  border-bottom: 5px solid transparent;
  border-right: 6px solid var(--diagram-arrow);
}
.seq-message.dashed .seq-arrow {
  background: none;
  border-top: 2px dashed var(--diagram-arrow);
}
.seq-msg-body {
  font-family: var(--diagram-label-font);
  font-size: 0.75rem;
  color: var(--vp-c-text-3);
  margin: 0;
  white-space: pre;
}
```

**Usage examples**:

Stage handshake (`tcp.md`):
```html
<SequenceDiagram
  :left="{ label: 'Server', highlight: 'brand' }"
  :right="{ label: 'Addon (Godot)', highlight: 'amber' }"
  :messages="[
    { from: 'left', to: 'right', label: 'TCP connect (port 9077)' },
    { from: 'right', to: 'left', label: 'handshake',
      body: '{ type: \"handshake\", version, godot_version, project }' },
    { from: 'left', to: 'right', label: 'handshake_ack',
      body: '{ type: \"handshake_ack\", version }' },
    { from: 'right', to: 'left', label: 'request / response',
      style: 'dashed', note: 'normal operation' },
  ]"
/>
```

Persistent request-response (`tcp.md`):
```html
<SequenceDiagram
  :left="{ label: 'Server' }"
  :right="{ label: 'Addon' }"
  :messages="[
    { from: 'left', to: 'right', label: 'Tool call 1: request → response' },
    { from: 'left', to: 'right', label: 'Tool call 2: request → response' },
    { from: 'left', to: 'right', label: 'Tool call 3: request → response' },
  ]"
/>
```

**Acceptance Criteria**:
- [ ] Renders two actor boxes with vertical lifelines
- [ ] Arrows point in the correct direction (left→right or right→left)
- [ ] `body` text renders below the arrow in monospace
- [ ] `dashed` style renders a dashed arrow line
- [ ] Actor `highlight` applies a colored bottom border
- [ ] Dark and light theme compatible
- [ ] Minimum width prevents overlap; horizontal scroll on narrow screens

---

### Unit 3: `FrameDiagram.vue`

**File**: `site/.vitepress/theme/components/FrameDiagram.vue`

A stacked horizontal bar showing sections of a data format (like a TCP frame). Simple component — just labeled sections side by side.

```typescript
interface FrameSection {
  label: string
  detail: string
  flex?: number    // Relative width (default 1)
  highlight?: 'brand' | 'amber' | 'muted'
}

defineProps<{
  sections: FrameSection[]
  title?: string
}>()
```

**Rendering approach**:
- A single horizontal flexbox row where each section is a bordered cell
- `flex` controls relative widths (e.g., length header is narrow, payload is wide)
- Each section shows `label` (bold, top) and `detail` (small, bottom)
- Left section has rounded left corners, right section has rounded right corners

**Markup structure**:

```html
<div class="diagram">
  <div class="frame-title" v-if="title">{{ title }}</div>
  <div class="frame-row">
    <div v-for="(s, i) in sections" class="frame-section"
         :class="[s.highlight, { first: i === 0, last: i === sections.length - 1 }]"
         :style="{ flex: s.flex ?? 1 }">
      <span class="frame-label">{{ s.label }}</span>
      <span class="frame-detail">{{ s.detail }}</span>
    </div>
  </div>
</div>
```

**CSS sketch**:

```css
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
  border-left: none;  /* collapse shared borders */
}
.frame-section.first {
  border-radius: var(--diagram-box-radius) 0 0 var(--diagram-box-radius);
}
.frame-section.last {
  border-radius: 0 var(--diagram-box-radius) var(--diagram-box-radius) 0;
}
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
```

**Usage**:

TCP frame (`tcp.md`):
```html
<FrameDiagram :sections="[
  { label: 'length', detail: '4 bytes, BE u32', flex: 1 },
  { label: 'JSON payload', detail: 'N bytes', flex: 3 },
]" />
```

**Acceptance Criteria**:
- [ ] Renders a horizontal bar with labeled sections
- [ ] `flex` controls proportional widths
- [ ] First/last sections have rounded corners
- [ ] Dark and light theme compatible

---

### Unit 4: `DepGraph.vue`

**File**: `site/.vitepress/theme/components/DepGraph.vue`

A small directed dependency graph with manually positioned nodes. Since the crate graph is simple (5 nodes, ~4 edges), a CSS grid layout with SVG overlay arrows is cleaner than an auto-layout algorithm.

```typescript
interface DepNode {
  id: string
  label: string
  row: number      // Grid row (0-indexed)
  col: number      // Grid column (0-indexed)
  highlight?: 'brand' | 'amber' | 'muted'
  note?: string    // Small annotation e.g. "(no stage deps)"
}

interface DepEdge {
  from: string     // Node id
  to: string       // Node id
  label?: string
}

defineProps<{
  nodes: DepNode[]
  edges: DepEdge[]
  cols?: number     // Grid columns (default 3)
  rows?: number     // Grid rows (default 3)
}>()
```

**Rendering approach**:
- CSS grid positions node boxes
- An SVG overlay (position: absolute, pointer-events: none) draws edges between nodes
- Edge coordinates are computed from node grid positions (center of each cell)
- SVG arrows use `<line>` with `marker-end` arrowheads
- `onMounted` + `ResizeObserver` to recompute SVG coordinates when layout changes

**Implementation notes**:
- Use `ref` to get DOM element positions via `getBoundingClientRect()` relative to the diagram container
- Each node has a `data-node-id` attribute; the SVG logic finds source/target bounding boxes and draws center-to-center lines
- Arrowhead defined as an SVG `<defs><marker>` reused by all lines
- Edge labels rendered as SVG `<text>` at the midpoint of each line

**Usage**:

Crate dependency graph (`crates.md`):
```html
<DepGraph
  :cols="3" :rows="3"
  :nodes="[
    { id: 'godot', label: 'stage-godot', row: 0, col: 0, highlight: 'amber' },
    { id: 'protocol', label: 'stage-protocol', row: 1, col: 1 },
    { id: 'server', label: 'stage-server', row: 0, col: 2, highlight: 'brand' },
    { id: 'core', label: 'stage-core', row: 2, col: 1 },
    { id: 'director', label: 'director', row: 2, col: 0, note: 'no stage deps', highlight: 'muted' },
    { id: 'cli', label: 'theatre-cli', row: 2, col: 2, note: 'clap + filesystem', highlight: 'muted' },
  ]"
  :edges="[
    { from: 'godot', to: 'protocol' },
    { from: 'server', to: 'protocol' },
    { from: 'server', to: 'core' },
  ]"
/>
```

**Acceptance Criteria**:
- [ ] Renders nodes in a CSS grid with correct positioning
- [ ] SVG arrows connect correct source/target nodes
- [ ] Arrowheads point at the target node
- [ ] SVG recomputes on window resize
- [ ] `note` text renders below node label in smaller font
- [ ] `highlight` applies a colored border accent
- [ ] Dark and light theme compatible

---

### Unit 5: Restyle `ArchDiagram.vue`

**File**: `site/.vitepress/theme/components/ArchDiagram.vue`

Restyle the existing homepage diagram to use the shared diagram tokens. The structure stays the same (two-path split showing Stage and Director) but switches from ASCII in `<pre>` to styled HTML boxes matching the new components.

**Approach**: Rewrite the template to use `<div>` boxes with the same CSS classes as `FlowDiagram` (`.flow-step`, `.flow-connector`), but with a custom two-column layout for the Stage/Director split. Keep the `highlight` prop for toggling accent colors.

This is a visual restyle, not a refactor into FlowDiagram — the two-column fork makes it structurally different from the single-column FlowDiagram.

**Acceptance Criteria**:
- [ ] Visual appearance matches the new diagram system (same colors, borders, fonts)
- [ ] `highlight` prop still works (brand for Stage, amber for Director)
- [ ] No ASCII box-drawing characters remain

---

### Unit 6: Integration — Replace ASCII in markdown pages

**Files modified**:
- `site/guide/how-it-works.md` — replace ASCII with `<FlowDiagram>`
- `site/architecture/tcp.md` — replace 4 ASCII diagrams with `<FrameDiagram>`, `<SequenceDiagram>`, `<FlowDiagram>`
- `site/architecture/crates.md` — replace ASCII with `<DepGraph>`

**Acceptance Criteria**:
- [ ] No ASCII diagrams remain in `site/` markdown files
- [ ] All diagram components render correctly in `vitepress dev`
- [ ] Pages still read naturally — surrounding prose unchanged

---

### Unit 7: Register components in theme

**File**: `site/.vitepress/theme/index.ts`

Add imports and register global components:

```typescript
import FlowDiagram from './components/FlowDiagram.vue'
import SequenceDiagram from './components/SequenceDiagram.vue'
import FrameDiagram from './components/FrameDiagram.vue'
import DepGraph from './components/DepGraph.vue'

// In enhanceApp:
app.component('FlowDiagram', FlowDiagram)
app.component('SequenceDiagram', SequenceDiagram)
app.component('FrameDiagram', FrameDiagram)
app.component('DepGraph', DepGraph)
```

**Acceptance Criteria**:
- [ ] All four new components are globally registered
- [ ] Components can be used in any `.md` file without explicit imports

---

## Implementation Order

1. **Diagram tokens** in `custom.css` — shared foundation
2. **FlowDiagram** — most used (3 diagrams), simplest to implement
3. **FrameDiagram** — simplest component overall, 1 diagram
4. **SequenceDiagram** — moderate complexity, 2 diagrams
5. **DepGraph** — most complex (SVG overlay), 1 diagram
6. **Restyle ArchDiagram** — adapt existing component
7. **Register components** in theme `index.ts`
8. **Replace ASCII** in markdown pages

Steps 2–5 can be parallelized since the components are independent.

## Testing

### Visual verification

```bash
cd site && npx vitepress dev
```

Check each page in both dark and light mode:
- `http://localhost:5173/` (homepage — ArchDiagram)
- `http://localhost:5173/guide/how-it-works` (FlowDiagram)
- `http://localhost:5173/architecture/tcp` (FrameDiagram, SequenceDiagram, FlowDiagram)
- `http://localhost:5173/architecture/crates` (DepGraph)

### Responsive check

Resize browser to 375px width — all diagrams should either scale down gracefully or scroll horizontally (no overflow/clipping).

### Build verification

```bash
cd site && npx vitepress build
```

No build errors, no missing component warnings.

## Verification Checklist

- [ ] `npx vitepress build` succeeds with no errors
- [ ] All 8 ASCII diagrams replaced with Vue components
- [ ] Diagrams render in dark mode
- [ ] Diagrams render in light mode
- [ ] No ASCII box-drawing characters (`─│┌┐└┘`) remain in `site/**/*.md` (except inside code blocks demonstrating wire format)
- [ ] Existing content/prose around diagrams unchanged
