<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'

interface DepNode {
  id: string
  label: string
  row: number
  col: number
  highlight?: 'brand' | 'amber' | 'muted'
  note?: string
}

interface DepEdge {
  from: string
  to: string
  label?: string
}

interface EdgeLine {
  x1: number
  y1: number
  x2: number
  y2: number
  label?: string
  mx: number
  my: number
}

const props = defineProps<{
  nodes: DepNode[]
  edges: DepEdge[]
  cols?: number
  rows?: number
}>()

const containerRef = ref<HTMLElement | null>(null)
const nodeRefs = ref<Record<string, HTMLElement>>({})
const edgeLines = ref<EdgeLine[]>([])
const svgWidth = ref(0)
const svgHeight = ref(0)

function setNodeRef(id: string, el: HTMLElement | null) {
  if (el) nodeRefs.value[id] = el
}

function computeEdges() {
  if (!containerRef.value) return
  const containerRect = containerRef.value.getBoundingClientRect()
  svgWidth.value = containerRect.width
  svgHeight.value = containerRect.height

  const lines: EdgeLine[] = []
  for (const edge of props.edges) {
    const fromEl = nodeRefs.value[edge.from]
    const toEl = nodeRefs.value[edge.to]
    if (!fromEl || !toEl) continue

    const fromRect = fromEl.getBoundingClientRect()
    const toRect = toEl.getBoundingClientRect()

    const x1 = fromRect.left + fromRect.width / 2 - containerRect.left
    const y1 = fromRect.top + fromRect.height / 2 - containerRect.top
    const x2 = toRect.left + toRect.width / 2 - containerRect.left
    const y2 = toRect.top + toRect.height / 2 - containerRect.top

    // Adjust endpoint to stop at target box border
    const dx = x2 - x1
    const dy = y2 - y1
    const dist = Math.sqrt(dx * dx + dy * dy)
    const halfW = toRect.width / 2
    const halfH = toRect.height / 2
    const t = Math.min(halfW / Math.abs(dx || 0.001), halfH / Math.abs(dy || 0.001))
    const ax2 = x2 - dx * t * (dist > 0 ? 1 / dist * Math.min(dist, halfW < halfH ? halfW : halfH) : 0)
    const ay2 = y2 - dy * t * (dist > 0 ? 1 / dist * Math.min(dist, halfW < halfH ? halfW : halfH) : 0)

    // Simpler: just clip to box edge
    let ex = x2
    let ey = y2
    if (dist > 0) {
      const ux = dx / dist
      const uy = dy / dist
      // find intersection with target box
      const txW = halfW / (Math.abs(ux) || 0.0001)
      const txH = halfH / (Math.abs(uy) || 0.0001)
      const tEdge = Math.min(txW, txH)
      ex = x2 - ux * tEdge
      ey = y2 - uy * tEdge
    }

    lines.push({
      x1,
      y1,
      x2: ex,
      y2: ey,
      label: edge.label,
      mx: (x1 + ex) / 2,
      my: (y1 + ey) / 2,
    })
  }
  edgeLines.value = lines
}

let observer: ResizeObserver | null = null

onMounted(async () => {
  await nextTick()
  computeEdges()
  if (containerRef.value) {
    observer = new ResizeObserver(() => computeEdges())
    observer.observe(containerRef.value)
  }
})

onUnmounted(() => {
  observer?.disconnect()
})
</script>

<template>
  <div class="diagram dep-graph" ref="containerRef">
    <div
      class="dep-grid"
      :style="{
        gridTemplateColumns: `repeat(${cols ?? 3}, 1fr)`,
        gridTemplateRows: `repeat(${rows ?? 3}, auto)`,
      }"
    >
      <div
        v-for="node in nodes"
        :key="node.id"
        class="dep-node"
        :class="node.highlight"
        :ref="(el) => setNodeRef(node.id, el as HTMLElement | null)"
        :style="{
          gridColumn: (node.col + 1),
          gridRow: (node.row + 1),
        }"
      >
        <span class="dep-label">{{ node.label }}</span>
        <span class="dep-note" v-if="node.note">{{ node.note }}</span>
      </div>
    </div>
    <svg
      class="dep-svg"
      :width="svgWidth"
      :height="svgHeight"
      aria-hidden="true"
    >
      <defs>
        <marker
          id="arrowhead"
          markerWidth="8"
          markerHeight="6"
          refX="7"
          refY="3"
          orient="auto"
        >
          <polygon points="0 0, 8 3, 0 6" class="dep-arrow-marker" />
        </marker>
      </defs>
      <g v-for="(line, i) in edgeLines" :key="i">
        <line
          :x1="line.x1"
          :y1="line.y1"
          :x2="line.x2"
          :y2="line.y2"
          class="dep-edge"
          marker-end="url(#arrowhead)"
        />
        <text
          v-if="line.label"
          :x="line.mx"
          :y="line.my - 4"
          class="dep-edge-label"
          text-anchor="middle"
        >{{ line.label }}</text>
      </g>
    </svg>
  </div>
</template>

<style scoped>
.dep-graph {
  position: relative;
}
.dep-grid {
  display: grid;
  gap: 1.5rem;
  padding: 0.5rem;
}
.dep-node {
  background: var(--diagram-box-bg);
  border: 1px solid var(--diagram-box-border);
  border-radius: var(--diagram-box-radius);
  padding: 0.75rem 1rem;
  text-align: center;
  min-width: 120px;
}
.dep-node.brand { border-left: 3px solid var(--diagram-accent); }
.dep-node.amber { border-left: 3px solid var(--diagram-accent-alt); }
.dep-node.muted { opacity: 0.65; }

.dep-label {
  display: block;
  font-family: var(--diagram-label-font);
  font-size: 0.82rem;
  font-weight: 600;
  color: var(--diagram-title);
}
.dep-note {
  display: block;
  font-size: 0.72rem;
  color: var(--diagram-label);
  margin-top: 0.2rem;
}

.dep-svg {
  position: absolute;
  top: 0;
  left: 0;
  pointer-events: none;
  overflow: visible;
}
.dep-edge {
  stroke: var(--diagram-arrow);
  stroke-width: 1.5;
  fill: none;
}
.dep-arrow-marker {
  fill: var(--diagram-arrow);
}
.dep-edge-label {
  fill: var(--diagram-label);
  font-family: var(--diagram-label-font);
  font-size: 0.7rem;
}
</style>
