# Design: Theatre GitHub Pages Documentation Site

## Overview

A VitePress-powered documentation and marketing site for Theatre, deployed to `godot-theatre.dev` via GitHub Pages. The site blends Godot's visual identity with theatre/stage metaphors to create a distinctive, gamer-friendly presence that tells the story of AI-assisted game development.

**Theme**: "The stage is set" — Godot's blue meets theatrical warmth. Dark-first, professional but approachable. Code-heavy examples driven by real game development pain points.

---

## Implementation Units

### Unit 1: VitePress Scaffold & GitHub Pages Deploy

**File**: `site/package.json`

```json
{
  "name": "theatre-docs",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vitepress dev",
    "build": "vitepress build",
    "preview": "vitepress preview"
  },
  "devDependencies": {
    "vitepress": "^1.6.0",
    "vue": "^3.5.0"
  }
}
```

**File**: `site/.vitepress/config.mts`

```typescript
import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Theatre',
  description: 'AI agent toolkit for building and debugging Godot games',
  head: [
    ['link', { rel: 'icon', href: '/favicon.svg' }],
    ['meta', { property: 'og:title', content: 'Theatre — AI Toolkit for Godot' }],
    ['meta', { property: 'og:description', content: 'Give your AI agent eyes into your running Godot game.' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:url', content: 'https://godot-theatre.dev' }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
  ],

  // Custom domain
  // CNAME file placed in site/public/CNAME

  themeConfig: {
    logo: '/logo.svg',
    siteTitle: 'Theatre',

    nav: [
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'Stage', link: '/stage/' },
      { text: 'Director', link: '/director/' },
      { text: 'Examples', link: '/examples/' },
      {
        text: 'More',
        items: [
          { text: 'API Reference', link: '/api/' },
          { text: 'Changelog', link: '/changelog' },
          { text: 'Architecture', link: '/architecture/' },
        ]
      }
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Getting Started',
          items: [
            { text: 'What is Theatre?', link: '/guide/what-is-theatre' },
            { text: 'Installation', link: '/guide/installation' },
            { text: 'Quick Start', link: '/guide/getting-started' },
            { text: 'Your First Session', link: '/guide/first-session' },
          ]
        },
        {
          text: 'Core Concepts',
          items: [
            { text: 'How It Works', link: '/guide/how-it-works' },
            { text: 'MCP & Your AI Agent', link: '/guide/mcp-basics' },
            { text: 'Token Budgets', link: '/guide/token-budgets' },
          ]
        }
      ],

      '/stage/': [
        {
          text: 'Stage',
          items: [
            { text: 'Overview', link: '/stage/' },
            { text: 'Spatial Snapshot', link: '/stage/snapshot' },
            { text: 'Spatial Delta', link: '/stage/delta' },
            { text: 'Spatial Query', link: '/stage/query' },
            { text: 'Spatial Inspect', link: '/stage/inspect' },
            { text: 'Spatial Watch', link: '/stage/watch' },
            { text: 'Spatial Config', link: '/stage/config' },
            { text: 'Spatial Action', link: '/stage/action' },
            { text: 'Scene Tree', link: '/stage/scene-tree' },
            { text: 'Recording', link: '/stage/recording' },
          ]
        },
        {
          text: 'Workflows',
          items: [
            { text: 'The Dashcam', link: '/stage/dashcam' },
            { text: 'Watch & React', link: '/stage/watch-workflow' },
            { text: 'Editor Dock', link: '/stage/editor-dock' },
          ]
        }
      ],

      '/director/': [
        {
          text: 'Director',
          items: [
            { text: 'Overview', link: '/director/' },
            { text: 'Scene Operations', link: '/director/scenes' },
            { text: 'Node Manipulation', link: '/director/nodes' },
            { text: 'Resources', link: '/director/resources' },
            { text: 'TileMap & GridMap', link: '/director/tilemaps' },
            { text: 'Animation', link: '/director/animation' },
            { text: 'Shaders', link: '/director/shaders' },
            { text: 'Physics Layers', link: '/director/physics' },
            { text: 'Scene Wiring', link: '/director/wiring' },
            { text: 'Batch Operations', link: '/director/batch' },
          ]
        },
        {
          text: 'Backends',
          items: [
            { text: 'Editor Plugin', link: '/director/editor-backend' },
            { text: 'Headless Daemon', link: '/director/daemon' },
          ]
        }
      ],

      '/examples/': [
        {
          text: 'Debugging Scenarios',
          items: [
            { text: 'Overview', link: '/examples/' },
            { text: 'Physics Tunneling', link: '/examples/physics-tunneling' },
            { text: 'Pathfinding Failures', link: '/examples/pathfinding' },
            { text: 'Animation Sync', link: '/examples/animation-sync' },
            { text: 'Collision Layer Bugs', link: '/examples/collision-layers' },
            { text: 'UI Overlap & Layout', link: '/examples/ui-overlap' },
          ]
        },
        {
          text: 'Building Scenarios',
          items: [
            { text: 'Level From Scratch', link: '/examples/build-level' },
            { text: 'Director + Stage Loop', link: '/examples/build-verify' },
          ]
        }
      ],

      '/api/': [
        {
          text: 'API Reference',
          items: [
            { text: 'Stage Tools', link: '/api/' },
            { text: 'Director Tools', link: '/api/director' },
            { text: 'Wire Format', link: '/api/wire-format' },
            { text: 'Error Codes', link: '/api/errors' },
          ]
        }
      ],

      '/architecture/': [
        {
          text: 'Architecture',
          items: [
            { text: 'Overview', link: '/architecture/' },
            { text: 'Crate Structure', link: '/architecture/crates' },
            { text: 'TCP Protocol', link: '/architecture/tcp' },
            { text: 'Contributing', link: '/architecture/contributing' },
          ]
        }
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/user/theatre' }
    ],

    search: {
      provider: 'local'
    },

    footer: {
      message: 'Open source under the MIT License.',
      copyright: 'Theatre — AI toolkit for Godot'
    },

    editLink: {
      pattern: 'https://github.com/user/theatre/edit/main/site/:path',
      text: 'Edit this page on GitHub'
    }
  }
})
```

**File**: `.github/workflows/deploy-pages.yml`

```yaml
name: Deploy Pages
on:
  push:
    branches: [main]
    paths: ['site/**']
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: site/package-lock.json
      - run: npm ci
        working-directory: site
      - run: npm run build
        working-directory: site
      - uses: actions/upload-pages-artifact@v3
        with:
          path: site/.vitepress/dist

  deploy:
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    needs: build
    runs-on: ubuntu-latest
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

**File**: `site/public/CNAME`

```
godot-theatre.dev
```

**Implementation Notes**:
- All site content lives under `site/` to keep it separate from the Rust workspace
- VitePress builds to `site/.vitepress/dist`
- GitHub Pages deployment triggers only on changes to `site/` directory
- CNAME file in `public/` gets copied to dist root for custom domain

**Acceptance Criteria**:
- [ ] `npm run dev` serves the site locally at localhost:5173
- [ ] `npm run build` produces static output in `.vitepress/dist`
- [ ] GitHub Actions workflow deploys on push to main when `site/` changes
- [ ] CNAME file present in build output for custom domain

---

### Unit 2: Custom Theme — "Godot Theatre"

**File**: `site/.vitepress/theme/index.ts`

```typescript
import type { Theme } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import './custom.css'
import ToolCard from './components/ToolCard.vue'
import ScenarioCard from './components/ScenarioCard.vue'
import AgentConversation from './components/AgentConversation.vue'
import ArchDiagram from './components/ArchDiagram.vue'
import HeroSection from './components/HeroSection.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('ToolCard', ToolCard)
    app.component('ScenarioCard', ScenarioCard)
    app.component('AgentConversation', AgentConversation)
    app.component('ArchDiagram', ArchDiagram)
    app.component('HeroSection', HeroSection)
  }
} satisfies Theme
```

**File**: `site/.vitepress/theme/custom.css`

Color system blending Godot blue with theatrical warmth:

```css
/* === Godot Theatre Color System === */

:root {
  /* Godot blue as primary brand */
  --vp-c-brand-1: #478CBF;
  --vp-c-brand-2: #5A9FD4;
  --vp-c-brand-3: #6DB2E8;
  --vp-c-brand-soft: rgba(71, 140, 191, 0.14);

  /* Theatre warm accent — stage amber */
  --theatre-amber: #D4A05A;
  --theatre-amber-soft: rgba(212, 160, 90, 0.14);
  --theatre-amber-dim: #A67C3D;

  /* Spotlight highlight for emphasis */
  --theatre-spotlight: #F5E6CC;

  /* Curtain red for warnings/destructive */
  --theatre-curtain: #C45B5B;

  /* Scene green for success states */
  --theatre-scene: #5BC46B;

  /* Typography */
  --vp-font-family-base: 'Inter', ui-sans-serif, system-ui, -apple-system,
    BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', sans-serif;
  --vp-font-family-mono: 'JetBrains Mono', ui-monospace, 'Cascadia Code',
    'Source Code Pro', Menlo, Monaco, Consolas, monospace;
}

/* Dark mode (default) */
.dark {
  --vp-c-bg: #1a1d23;
  --vp-c-bg-alt: #14161a;
  --vp-c-bg-soft: #1e2128;
  --vp-c-bg-elv: #22252b;

  /* Slightly warm-tinted text (not pure white — theatrical) */
  --vp-c-text-1: #e8e4df;
  --vp-c-text-2: #a8a49e;
  --vp-c-text-3: #6e6b66;

  /* Borders with subtle warmth */
  --vp-c-border: #33302c;
  --vp-c-divider: #2a2825;
  --vp-c-gutter: #1e1c1a;
}

/* Light mode override */
:root:not(.dark) {
  --vp-c-bg: #faf8f5;
  --vp-c-bg-alt: #f2efe9;
  --vp-c-bg-soft: #edeae4;

  --vp-c-brand-1: #3A7AAD;
  --vp-c-brand-2: #478CBF;
  --vp-c-brand-3: #5A9FD4;
}

/* Hero section styling */
.VPHero .VPImage {
  filter: drop-shadow(0 0 40px rgba(71, 140, 191, 0.3));
}

/* Code blocks: dark stage background */
.dark .vp-code-group,
.dark div[class*='language-'] {
  --vp-code-block-bg: #12141a;
}

/* Custom container: "spotlight" tip boxes */
.custom-block.tip {
  border-color: var(--theatre-amber);
}

.custom-block.tip .custom-block-title {
  color: var(--theatre-amber);
}

/* Tool cards grid */
.tool-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 1.5rem;
  margin: 2rem 0;
}

/* Scenario cards */
.scenario-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 1.5rem;
  margin: 2rem 0;
}

/* Agent conversation component */
.agent-conversation {
  border: 1px solid var(--vp-c-border);
  border-radius: 12px;
  overflow: hidden;
  margin: 1.5rem 0;
  background: var(--vp-c-bg-alt);
}

.agent-conversation .message {
  padding: 1rem 1.25rem;
  border-bottom: 1px solid var(--vp-c-divider);
  font-size: 0.9rem;
  line-height: 1.6;
}

.agent-conversation .message:last-child {
  border-bottom: none;
}

.agent-conversation .message.human {
  background: var(--vp-c-bg-soft);
}

.agent-conversation .message.human::before {
  content: '🎮 Developer';
  font-weight: 600;
  display: block;
  margin-bottom: 0.4rem;
  color: var(--theatre-amber);
  font-size: 0.8rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.agent-conversation .message.agent::before {
  content: '🤖 AI Agent';
  font-weight: 600;
  display: block;
  margin-bottom: 0.4rem;
  color: var(--vp-c-brand-1);
  font-size: 0.8rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.agent-conversation .message.tool {
  background: var(--vp-code-block-bg, #12141a);
  font-family: var(--vp-font-family-mono);
  font-size: 0.8rem;
  padding: 0.75rem 1.25rem;
  color: var(--vp-c-text-2);
}

.agent-conversation .message.tool::before {
  content: '→ MCP Tool Call';
  font-weight: 600;
  display: block;
  margin-bottom: 0.4rem;
  color: var(--theatre-scene);
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

/* Architecture diagram */
.arch-diagram {
  padding: 2rem;
  background: var(--vp-c-bg-alt);
  border-radius: 12px;
  border: 1px solid var(--vp-c-border);
  margin: 2rem 0;
  font-family: var(--vp-font-family-mono);
  font-size: 0.85rem;
  line-height: 1.8;
  text-align: center;
  overflow-x: auto;
}
```

**Implementation Notes**:
- Dark mode is the default (gamers expect dark mode)
- Godot blue (`#478CBF`) as primary brand color (recognition)
- Theatre amber (`#D4A05A`) as warm accent — stage lighting feel
- Background has warm undertone (not pure gray — `#1a1d23` leans slightly warm)
- Text is warm off-white (`#e8e4df`) instead of pure white — easier on eyes, theatrical
- JetBrains Mono for code (common in gamedev tooling)
- Light mode available but secondary — uses slightly parchment-toned backgrounds

**Acceptance Criteria**:
- [ ] Dark mode renders with Godot blue brand and warm theatrical tones
- [ ] Light mode is readable and maintains brand identity
- [ ] Code blocks have distinct dark stage background
- [ ] Custom tip blocks use amber accent
- [ ] All VitePress default components remain functional

---

### Unit 3: Vue Components

**File**: `site/.vitepress/theme/components/ToolCard.vue`

```vue
<script setup lang="ts">
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
  <a :href="link" class="tool-card">
    <div class="tool-card-icon">{{ icon }}</div>
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
```

**File**: `site/.vitepress/theme/components/ScenarioCard.vue`

```vue
<script setup lang="ts">
defineProps<{
  title: string
  problem: string    // 1-line problem statement
  icon: string
  link: string
}>()
</script>

<template>
  <a :href="link" class="scenario-card">
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
```

**File**: `site/.vitepress/theme/components/AgentConversation.vue`

```vue
<script setup lang="ts">
/**
 * Renders a simulated human-agent-tool conversation.
 *
 * Usage in markdown:
 * <AgentConversation :messages="[
 *   { role: 'human', text: 'My character clips through the east wall.' },
 *   { role: 'agent', text: 'Let me check what\\'s near that wall...' },
 *   { role: 'tool', text: 'spatial_query { type: \"radius\", origin: \"player\", radius: 5 }' },
 *   { role: 'agent', text: 'I can see the wall\\'s collision shape...' },
 * ]" />
 */
defineProps<{
  messages: Array<{
    role: 'human' | 'agent' | 'tool'
    text: string
  }>
}>()
</script>

<template>
  <div class="agent-conversation">
    <div
      v-for="(msg, i) in messages"
      :key="i"
      :class="['message', msg.role]"
    >
      {{ msg.text }}
    </div>
  </div>
</template>
```

**File**: `site/.vitepress/theme/components/ArchDiagram.vue`

```vue
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
  Stage (MCP)        Director (MCP)
  <em>observe the game</em>      <em>build the game</em>
       │                      │
  ┌────▼────────┐      ┌──────▼──────┐
  │  stage  │      │  director   │
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
.arch-pre.stage em:first-of-type,
.arch-pre.both em {
  color: var(--vp-c-brand-1);
}
.arch-pre.director em:last-of-type,
.arch-pre.both em {
  color: var(--theatre-amber);
}
</style>
```

**File**: `site/.vitepress/theme/components/HeroSection.vue`

```vue
<script setup lang="ts">
/**
 * Custom hero for the landing page, replacing VitePress default hero.
 * Shows the tagline with a typed-out agent conversation demo below.
 */
</script>

<template>
  <div class="hero-container">
    <div class="hero-content">
      <div class="hero-badge">MCP Toolkit for Godot</div>
      <h1 class="hero-title">
        Your AI can read your code.<br/>
        <span class="hero-highlight">Now it can see your game.</span>
      </h1>
      <p class="hero-tagline">
        Theatre gives AI agents spatial awareness of running Godot games
        and the ability to build scenes, resources, and animations — all through
        the Model Context Protocol.
      </p>
      <div class="hero-actions">
        <a href="/guide/getting-started" class="hero-btn primary">Get Started</a>
        <a href="/guide/what-is-theatre" class="hero-btn secondary">Learn More</a>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hero-container {
  max-width: 800px;
  margin: 0 auto;
  padding: 4rem 1.5rem 3rem;
  text-align: center;
}
.hero-badge {
  display: inline-block;
  padding: 0.3rem 1rem;
  border-radius: 99px;
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
  font-size: 0.85rem;
  font-weight: 500;
  margin-bottom: 1.5rem;
  letter-spacing: 0.02em;
}
.hero-title {
  font-size: 3rem;
  font-weight: 700;
  line-height: 1.2;
  margin: 0 0 1.5rem;
  color: var(--vp-c-text-1);
}
.hero-highlight {
  background: linear-gradient(135deg, var(--vp-c-brand-1), var(--theatre-amber));
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}
.hero-tagline {
  font-size: 1.15rem;
  color: var(--vp-c-text-2);
  line-height: 1.7;
  margin: 0 0 2rem;
  max-width: 600px;
  margin-left: auto;
  margin-right: auto;
}
.hero-actions {
  display: flex;
  gap: 1rem;
  justify-content: center;
  flex-wrap: wrap;
}
.hero-btn {
  padding: 0.75rem 2rem;
  border-radius: 8px;
  font-weight: 600;
  font-size: 1rem;
  text-decoration: none;
  transition: opacity 0.2s, transform 0.15s;
}
.hero-btn:hover { transform: translateY(-1px); }
.hero-btn.primary {
  background: var(--vp-c-brand-1);
  color: #fff;
}
.hero-btn.secondary {
  border: 1px solid var(--vp-c-border);
  color: var(--vp-c-text-1);
  background: var(--vp-c-bg-soft);
}

@media (max-width: 640px) {
  .hero-title { font-size: 2rem; }
  .hero-tagline { font-size: 1rem; }
}
</style>
```

**Acceptance Criteria**:
- [ ] ToolCard renders with icon, title, description, tool name, and token cost
- [ ] ScenarioCard renders with hover effect transitioning to amber border
- [ ] AgentConversation renders human/agent/tool messages with distinct styling
- [ ] ArchDiagram renders ASCII architecture with optional highlight
- [ ] HeroSection renders gradient title text, badge, and CTA buttons
- [ ] All components are registered globally via theme/index.ts

---

### Unit 4: Landing Page

**File**: `site/index.md`

```markdown
---
layout: home
hero: false
---

<HeroSection />

## The Problem

AI coding agents can read your source files, set breakpoints, inspect variables — but they
**cannot see your game**. When an enemy clips through a wall, when a patrol path overshoots,
when physics bodies tunnel through geometry — your agent has no way to observe these problems.

It's like debugging a web app without being able to open the browser.

## Two Tools, One Stage

<div class="tool-cards">
<ToolCard
  title="Stage"
  icon="🔭"
  description="Observe the running game. Spatial snapshots, real-time deltas, watches, recordings. Your AI sees what the player sees — as structured data."
  tool="9 MCP tools"
  tokens="200-1500"
  link="/stage/"
/>
<ToolCard
  title="Director"
  icon="🎬"
  description="Build and modify scenes, resources, tilemaps, and animations through Godot's own API. No hand-editing .tscn files."
  tool="25+ operations"
  tokens="—"
  link="/director/"
/>
</div>

## How It Works

<ArchDiagram highlight="both" />

Theatre connects your AI agent to your Godot game through the
**Model Context Protocol (MCP)**. Stage observes the running game via a
GDExtension addon. Director modifies scenes through the editor or headless Godot.
Both communicate over TCP, exposing structured tools your agent already knows
how to use.

## The Dashcam Moment

The killer workflow: **human plays, AI analyzes**.

<AgentConversation :messages="[
  { role: 'human', text: 'I just recorded a clip of the bug. The player clips through the east wall around frame 340.' },
  { role: 'agent', text: 'Let me analyze the recording around that frame.' },
  { role: 'tool', text: 'recording { action: \"query_range\", clip_id: \"clip_01\", start_frame: 330, end_frame: 350, condition: { type: \"proximity\", nodes: [\"player\", \"wall_east\"], max_distance: 0.5 } }' },
  { role: 'agent', text: 'Found it. At frame 337, the player\\'s velocity spikes to 45 units/frame — the CharacterBody3D tunnels through the wall\\'s collision shape. The move_and_slide delta is too large for the CollisionShape3D thickness (0.3 units). Two fixes: thicken the collision shape to 1.0, or enable continuous collision detection.' },
]" />

You press **F8** to record, **F9** to mark the bug moment, and the agent
scrubs through the spatial timeline to find exactly what went wrong — no
screenshots, no narration, no guessing from code.

## Real Debugging Scenarios

<div class="scenario-cards">
<ScenarioCard
  title="Physics Tunneling"
  icon="💥"
  problem="Fast-moving objects pass through walls. The code looks right but the collision just... doesn't happen."
  link="/examples/physics-tunneling"
/>
<ScenarioCard
  title="Pathfinding Failures"
  icon="🗺️"
  problem="NPCs get stuck, take bizarre routes, or refuse to move. The navmesh looks fine in the editor."
  link="/examples/pathfinding"
/>
<ScenarioCard
  title="Collision Layer Confusion"
  icon="🎭"
  problem="Two objects should collide but don't. Or they collide when they shouldn't. Layer/mask bits are a maze."
  link="/examples/collision-layers"
/>
<ScenarioCard
  title="Animation Sync Issues"
  icon="🎵"
  problem="The attack animation plays but the hitbox doesn't activate at the right frame. Timing is off."
  link="/examples/animation-sync"
/>
</div>

## Quick Start

### 1. Install Theatre

```bash
# Clone and build
git clone https://github.com/user/theatre
cd theatre
cargo build --workspace --release

# Deploy Stage to your Godot project
theatre-deploy --release ~/your-godot-project
```

### 2. Enable the addons

In Godot: **Project → Project Settings → Plugins → Stage → Enable**

For Director, copy `addons/director/` into your project and enable similarly.

### 3. Configure your AI agent

Add to your project's `.mcp.json`:

```json
{
  "mcpServers": {
    "stage": {
      "type": "stdio",
      "command": "/path/to/theatre/target/release/stage-server"
    },
    "director": {
      "type": "stdio",
      "command": "/path/to/theatre/target/release/director",
      "args": ["serve"]
    }
  }
}
```

### 4. Run your game and ask

```
"Take a spatial snapshot of my scene"
```

Your AI agent now sees your game world.
```

**Acceptance Criteria**:
- [ ] Landing page renders custom hero (not VitePress default)
- [ ] Two ToolCards display for Stage and Director
- [ ] Architecture diagram renders with both paths highlighted
- [ ] Dashcam conversation renders with human/agent/tool messages
- [ ] Four scenario cards render with links to examples
- [ ] Quick start section has copy-pasteable commands
- [ ] Page is responsive down to 375px width

---

### Unit 5: Stage Guide Pages

**File**: `site/stage/index.md` — Overview page

Content structure:
- What Stage does (2 paragraphs, derived from VISION.md "The Solution" section)
- The 9 tools as ToolCards in a grid (using data from CONTRACT.md)
- "When to use each tool" decision flowchart in prose:
  - Starting out? → `spatial_snapshot` with `detail: "summary"`
  - Something specific? → `spatial_query` or `spatial_inspect`
  - Tracking changes? → `spatial_watch` then `spatial_delta`
  - Reproducing a bug? → `recording`
  - Need to poke the game? → `spatial_action`
- Link to each tool's dedicated page

**Files**: `site/stage/{snapshot,delta,query,inspect,watch,config,action,scene-tree,recording}.md`

Each tool page follows this template:

```markdown
# spatial_snapshot

> Scene overview from a spatial perspective. The equivalent of opening a file —
> but for your running game.

## When to Use

{1-2 sentences: the mental model for when this tool is the right choice}

## Parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| ... | ... | ... | ... |

## Example: {Concrete scenario title}

<AgentConversation :messages="[...]" />

## Response Format

```json
{annotated example response from CONTRACT.md}
```

## Tips

- {Practical tip 1}
- {Practical tip 2}
```

**Files**: `site/stage/{dashcam,watch-workflow,editor-dock}.md`

Workflow pages that tell a story rather than document a single tool:
- **Dashcam**: The full recording→analyze workflow with a narrative walkthrough
- **Watch & React**: Setting up watches, reading deltas, the observe→act loop
- **Editor Dock**: What the dock shows, keyboard shortcuts (F8/F9/F10), how to read the activity feed

**Implementation Notes**:
- Tool pages source their parameter tables and response formats from CONTRACT.md
- Every tool page includes at least one AgentConversation showing a real workflow
- The "Tips" section captures non-obvious practical advice (e.g., "Use `detail: summary` first, drill down only when needed")
- Workflow pages cross-link to relevant tool pages

**Acceptance Criteria**:
- [ ] Stage overview lists all 9 tools with descriptions and token costs
- [ ] Each of the 9 tool pages has parameters, example conversation, response format
- [ ] Dashcam workflow page walks through F8→play→F9→analyze flow
- [ ] All pages render without errors and sidebar navigation works

---

### Unit 6: Director Guide Pages

**File**: `site/director/index.md` — Overview page

Content structure:
- What Director does (the .tscn/.tres problem, why agents can't hand-edit)
- Operations organized by domain (Scene, Node, Resource, TileMap, Animation, Shader, Physics, Wiring, Meta)
- Three backends explained simply (editor plugin → daemon → one-shot fallback)
- "You don't pick the backend" — Director auto-selects

**Files**: `site/director/{scenes,nodes,resources,tilemaps,animation,shaders,physics,wiring,batch,editor-backend,daemon}.md`

Each domain page follows:

```markdown
# Scene Operations

> Create, read, list, diff, and instance scenes — all through Godot's own API.

## Operations

### scene_create

{Description, parameters table, example}

### scene_read

{Description, parameters table, example}

...

## Example: {Concrete building scenario}

<AgentConversation :messages="[...]" />
```

**Implementation Notes**:
- Director pages are more reference-oriented than Stage (less narrative, more lookup)
- Batch page explains how to combine operations and why it matters (21 round-trips → 1)
- Backend pages are short — most users don't need to think about this

**Acceptance Criteria**:
- [ ] Director overview explains the .tscn problem clearly
- [ ] Each domain page documents all operations with parameters
- [ ] Backend pages explain editor vs daemon vs one-shot
- [ ] Batch page includes a before/after token comparison

---

### Unit 7: Example Scenario Pages

Each example page is a full debugging narrative. Structure:

```markdown
# Physics Tunneling

> Fast-moving objects pass through walls. The code looks fine.
> The collision shapes are there. But the ball just... goes through.

## The Setup

{Describe the game scenario — what the developer has, what goes wrong}

## The Investigation

<AgentConversation :messages="[
  // 6-10 messages showing the full debugging flow
  // Human describes problem → Agent uses Stage tools → diagnosis → fix
]" />

## What the Agent Found

{Explanation of the root cause with spatial data}

## The Fix

```gdscript
// The code change that fixes it
```

## Why This Works

{Brief explanation of the physics principle — e.g., CCD, collision shape thickness}

## Key Stage Tools Used

- `spatial_query` — checked proximity between ball and wall
- `recording` — scrubbed the timeline to find the exact tunneling frame
- `spatial_inspect` — examined the collision shape dimensions
```

**Files**:
- `site/examples/index.md` — Overview with all ScenarioCards
- `site/examples/physics-tunneling.md`
- `site/examples/pathfinding.md`
- `site/examples/animation-sync.md`
- `site/examples/collision-layers.md`
- `site/examples/ui-overlap.md`
- `site/examples/build-level.md` — Director-focused: building a level from scratch
- `site/examples/build-verify.md` — Full loop: Director builds, Stage verifies

**Implementation Notes**:
- Each debugging example should use realistic Godot class names (CharacterBody3D, NavigationAgent3D, Area3D, etc.)
- AgentConversation tool calls should use actual Stage/Director tool parameter format
- The "build-verify" example is the flagship cross-tool story
- Keep scenarios specific and relatable — these are problems every Godot dev has hit

**Acceptance Criteria**:
- [ ] Each scenario page tells a complete story from problem to fix
- [ ] AgentConversation tool calls use correct MCP tool parameter formats
- [ ] At least one example per page shows recording/dashcam workflow
- [ ] Build examples demonstrate Director operations with correct parameter format
- [ ] Examples index page shows all scenarios as ScenarioCards

---

### Unit 8: API Reference

**File**: `site/api/index.md` — Stage API Reference

Structured reference derived from CONTRACT.md. For each tool:
- Full parameter schema (every field, type, default, constraints)
- Full response schema
- Error codes specific to this tool
- No narrative — pure reference

**File**: `site/api/director.md` — Director API Reference

All Director operations with parameter schemas.

**File**: `site/api/wire-format.md`

- TCP length-prefix protocol
- JSON message format
- Handshake sequence
- Port configuration

**File**: `site/api/errors.md`

Complete error code table with descriptions and common causes.

**Implementation Notes**:
- API reference pages are generated-feeling but hand-written (no actual codegen for v1)
- Use VitePress `:::details` blocks for long response examples
- Cross-link to guide pages for narrative explanations

**Acceptance Criteria**:
- [ ] Every Stage tool parameter is documented with type and default
- [ ] Every Director operation is documented with parameters
- [ ] Error codes table is complete (matches CONTRACT.md)
- [ ] Wire format page documents the TCP protocol

---

### Unit 9: Changelog & Architecture Pages

**File**: `site/changelog.md`

```markdown
# Changelog

## Unreleased

### Stage
- ...

### Director
- ...

---

## v0.1.0 — Initial Release

{First public release notes}
```

**File**: `site/architecture/index.md`

High-level overview of crate structure, design decisions, ports & adapters approach.

**File**: `site/architecture/crates.md`

Crate dependency diagram, what each crate owns, the "thin addon, thick server" philosophy.

**File**: `site/architecture/tcp.md`

TCP protocol deep-dive for contributors.

**File**: `site/architecture/contributing.md`

How to build, test, contribute. Development workflow.

**Acceptance Criteria**:
- [ ] Changelog has a clear format for ongoing updates
- [ ] Architecture overview explains the key design decisions
- [ ] Contributing page has build/test/lint commands

---

### Unit 10: Static Assets & SEO

**File**: `site/public/favicon.svg`

Simple SVG combining a Godot-blue circle with a spotlight/theatre motif. Can be a placeholder initially.

**File**: `site/public/logo.svg`

Theatre logo for the nav bar. Text + icon mark.

**File**: `site/public/og-image.png`

Open Graph image (1200x630) for social sharing. Shows title + tagline + Theatre branding.

**Implementation Notes**:
- SVG favicons for crisp rendering at all sizes
- OG image should look good on Discord, Twitter, and GitHub link previews
- All assets in `site/public/` get copied to dist root

**Acceptance Criteria**:
- [ ] Favicon renders in browser tab
- [ ] Logo renders in nav bar at 24px height
- [ ] OG meta tags present in HTML head
- [ ] Social share preview shows title, description, and image

---

## Implementation Order

1. **Unit 1**: VitePress scaffold + deploy workflow — foundation everything depends on
2. **Unit 2**: Custom theme CSS — establishes the visual identity
3. **Unit 3**: Vue components — needed by all content pages
4. **Unit 10**: Static assets (placeholder SVGs) — needed for theme to render
5. **Unit 4**: Landing page — the first thing anyone sees
6. **Unit 5**: Stage guide pages — the primary content
7. **Unit 6**: Director guide pages — secondary content
8. **Unit 7**: Example scenario pages — the differentiator
9. **Unit 8**: API reference — the reference material
10. **Unit 9**: Changelog & architecture — supporting content

Units 5-8 can be parallelized once Units 1-4 are complete.

## Testing

### Local Development

```bash
cd site
npm install
npm run dev      # dev server with hot reload
npm run build    # production build (catches dead links, missing assets)
npm run preview  # preview production build locally
```

### Build Verification

```bash
# VitePress build will fail on:
# - Dead internal links (broken [[links]] or [text](missing-page))
# - Missing component imports
# - Vue template compilation errors

npm run build 2>&1 | grep -E "(error|warning)" || echo "Build clean"
```

### Manual Checks

- [ ] All sidebar links resolve to existing pages
- [ ] All cross-page links work
- [ ] Code blocks have correct language highlighting (json, bash, gdscript, rust)
- [ ] AgentConversation components render correctly
- [ ] Responsive layout works at 375px, 768px, 1280px, 1920px
- [ ] Dark mode toggle works
- [ ] Search finds content across all sections
- [ ] GitHub Pages deployment succeeds from CI

### Accessibility

- [ ] All images have alt text (when added)
- [ ] Color contrast meets WCAG AA (Godot blue on dark bg: 4.7:1 ratio ✓)
- [ ] Navigation is keyboard-accessible (VitePress default)
- [ ] Code blocks are scrollable, not clipped

## Verification Checklist

```bash
# Build the site
cd site && npm install && npm run build

# Check for dead links (VitePress reports these during build)
# Check the output for warnings

# Preview locally
npm run preview
# Visit http://localhost:4173

# Verify GitHub Actions workflow syntax
# (GitHub will validate on push)
```
