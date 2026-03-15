<script setup lang="ts">
/**
 * Custom hero for the landing page, replacing VitePress default hero.
 * Shows the tagline with a typed-out agent conversation demo below.
 */
import { withBase } from 'vitepress'
</script>

<template>
  <div class="hero-container">
    <div class="hero-content">
      <div class="hero-badge">MCP Toolkit for Godot</div>
      <h1 class="hero-title">
        Your AI can read your code.<br/>
        <span class="hero-highlight">Now it can see — and act in — your game.</span>
      </h1>
      <p class="hero-tagline">
        Theatre gives AI agents spatial awareness of running Godot games,
        live interaction for testing hypotheses, and the ability to build
        scenes, resources, and animations — all through the Model Context Protocol.
      </p>
      <div class="hero-actions">
        <a :href="withBase('/guide/getting-started')" class="hero-btn primary">Get Started</a>
        <a :href="withBase('/guide/what-is-theatre')" class="hero-btn secondary">Learn More</a>
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
