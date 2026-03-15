# Design: Comprehensive SEO Fixes for godot-theatre.dev

## Overview

Phased SEO implementation plan based on a full audit that scored the site **42/100**. The site has strong content and performance (VitePress SSG) but is missing nearly all SEO infrastructure: no robots.txt, no sitemap, no canonical tags, no structured data, no per-page meta descriptions, no llms.txt.

**Audit scores by category:**
| Category | Score |
|----------|-------|
| Technical SEO | 32/100 |
| Content Quality | 68/100 |
| On-Page SEO | 35/100 |
| Schema / Structured Data | 5/100 |
| Performance (CWV) | 78/100 |
| AI Search Readiness (GEO) | 52/100 |
| Images | 35/100 |

All changes are in `site/` (the VitePress documentation site).

---

## Phase 1: Crawl Infrastructure (Critical — < 1 hour)

Fixes that unblock search engine and AI crawler discovery. Pure config + static file changes.

### Unit 1.1: Enable sitemap generation

**File**: `site/.vitepress/config.mts`

Add `sitemap` to the top-level config:

```typescript
export default defineConfig({
  base: '/',
  title: 'Theatre',
  description: 'AI agent toolkit for building and debugging Godot games',

  sitemap: {
    hostname: 'https://godot-theatre.dev',
  },

  // ... rest of config unchanged
})
```

**Implementation Notes**:
- VitePress 1.x has built-in sitemap generation — no plugin needed
- Generates `sitemap.xml` at build time with all pages
- Must be placed at the top level of `defineConfig()`, not inside `themeConfig`

**Acceptance Criteria**:
- [ ] `npm run build` produces `sitemap.xml` in `.vitepress/dist/`
- [ ] Sitemap contains URLs for all ~45 content pages
- [ ] Each URL uses `https://godot-theatre.dev` as hostname

### Unit 1.2: Enable clean URLs

**File**: `site/.vitepress/config.mts`

Add `cleanUrls: true` to the top-level config:

```typescript
export default defineConfig({
  base: '/',
  cleanUrls: true,
  // ...
})
```

**Implementation Notes**:
- Removes `.html` extensions from all generated URLs
- VitePress will serve both `/guide/getting-started` and `/guide/getting-started.html` (the latter redirects)
- Internal sidebar/nav links in config already use clean URLs — this makes the build match

**Acceptance Criteria**:
- [ ] Built site uses clean URLs (no `.html` in sitemap entries)
- [ ] Navigation links resolve without `.html` suffix
- [ ] `npm run preview` serves pages at clean URLs

### Unit 1.3: Enable lastUpdated timestamps

**File**: `site/.vitepress/config.mts`

Add `lastUpdated: true` to the top-level config:

```typescript
export default defineConfig({
  base: '/',
  cleanUrls: true,
  lastUpdated: true,
  // ...
})
```

**Implementation Notes**:
- VitePress reads git commit timestamps per file
- Displays "Last updated: <date>" at the bottom of each page
- Adds freshness signals for E-E-A-T

**Acceptance Criteria**:
- [ ] Documentation pages show "Last updated" dates
- [ ] Dates reflect actual git history

### Unit 1.4: Add robots.txt

**File**: `site/public/robots.txt`

```
User-agent: *
Allow: /

User-agent: GPTBot
Allow: /

User-agent: OAI-SearchBot
Allow: /

User-agent: ClaudeBot
Allow: /

User-agent: PerplexityBot
Allow: /

User-agent: Google-Extended
Allow: /

Sitemap: https://godot-theatre.dev/sitemap.xml
```

**Implementation Notes**:
- Static file in `public/` — VitePress copies it to dist root
- Explicitly welcomes AI crawlers (default allow is implicit, but explicit is a positive signal)
- References the sitemap for crawler discovery

**Acceptance Criteria**:
- [ ] File is copied to `.vitepress/dist/robots.txt` on build
- [ ] Contains `Sitemap:` directive pointing to sitemap.xml

### Unit 1.5: Add llms.txt

**File**: `site/public/llms.txt`

```markdown
# Theatre

> AI agent toolkit for building and debugging Godot games via the Model Context Protocol (MCP).

Theatre gives AI agents spatial awareness of running Godot games, live interaction for testing hypotheses, and the ability to build scenes, resources, and animations — all through MCP tools.

## Key Pages

- [What is Theatre?](https://godot-theatre.dev/guide/what-is-theatre): Overview and core concepts
- [Getting Started](https://godot-theatre.dev/guide/getting-started): 10-minute quickstart guide
- [Installation](https://godot-theatre.dev/guide/installation): Install Theatre CLI and Godot addon
- [Stage Tools](https://godot-theatre.dev/stage/): 9 MCP tools for observing running games
- [Director Tools](https://godot-theatre.dev/director/): 38+ operations for building scenes
- [API Reference](https://godot-theatre.dev/api/): Complete tool schemas and wire format
- [Architecture](https://godot-theatre.dev/architecture/): System design and internals
- [Examples](https://godot-theatre.dev/examples/): Real debugging scenarios with session transcripts

## Core Concepts

- **Stage**: Observation layer — spatial snapshots, deltas, queries, watches, recordings, live property/method interaction
- **Director**: Build layer — create/modify scenes, nodes, resources, tilemaps, animations, shaders, physics layers
- **MCP**: Model Context Protocol — the standard interface AI agents use to call Theatre's tools
- **Dashcam**: Record last 60 seconds of spatial data, mark bug moments with F9, agent scrubs timeline

## Technical Details

- Godot GDExtension addon (Rust) communicates over TCP on port 9077
- Zero game performance impact (< 0.1ms/frame for 100 tracked nodes)
- MIT licensed, open source: https://github.com/nklisch/theatre
```

**Implementation Notes**:
- Follows the llms.txt specification (https://llmstxt.org/)
- Provides machine-readable summary for LLM consumption
- Static file in `public/`

**Acceptance Criteria**:
- [ ] File is served at `https://godot-theatre.dev/llms.txt`
- [ ] Contains project summary, key page links, and core concepts

---

## Phase 2: On-Page SEO Meta Tags (High — 1-2 hours)

Per-page meta descriptions, dynamic OG tags, and improved title.

### Unit 2.1: Improve homepage title

**File**: `site/.vitepress/config.mts`

Change the site-level `titleTemplate` to append a descriptor:

```typescript
export default defineConfig({
  base: '/',
  cleanUrls: true,
  lastUpdated: true,
  title: 'Theatre',
  titleTemplate: ':title — AI Toolkit for Godot',
  description: 'AI agent toolkit for building and debugging Godot games',
  // ...
})
```

**Implementation Notes**:
- Homepage will render as "Theatre — AI Toolkit for Godot"
- Subpages: "Getting Started — AI Toolkit for Godot"
- The `:title` token is replaced by each page's `title` frontmatter or H1

**Acceptance Criteria**:
- [ ] Homepage `<title>` tag is "Theatre — AI Toolkit for Godot"
- [ ] Subpage titles follow the pattern "{Page Title} — AI Toolkit for Godot"

### Unit 2.2: Add per-page descriptions via frontmatter

Add `description` frontmatter to all content pages. This sets `<meta name="description">` per page.

**Files and descriptions** (one frontmatter block per file):

```yaml
# site/guide/what-is-theatre.md
---
description: "Theatre is an AI agent toolkit that gives coding assistants spatial awareness of running Godot games via MCP tools."
---
```

```yaml
# site/guide/getting-started.md
---
description: "Get your first spatial snapshot in under 10 minutes. Connect your AI agent to a running Godot game with Theatre."
---
```

```yaml
# site/guide/installation.md
---
description: "Install Theatre CLI and the Godot GDExtension addon. Supports macOS, Linux, and Windows."
---
```

```yaml
# site/guide/first-session.md
---
description: "Walk through your first Theatre debugging session — observe game state, test hypotheses, and fix bugs with AI assistance."
---
```

```yaml
# site/guide/how-it-works.md
---
description: "How Theatre connects AI agents to Godot games — GDExtension addon, TCP protocol, and MCP tool architecture."
---
```

```yaml
# site/guide/mcp-basics.md
---
description: "Understanding the Model Context Protocol (MCP) and how AI agents use Theatre's tools to interact with Godot."
---
```

```yaml
# site/guide/token-budgets.md
---
description: "Managing AI context window costs with Theatre's token-efficient spatial tools. Each tool uses 200-3000 tokens."
---
```

```yaml
# site/stage/index.md
---
description: "Stage provides 9 MCP tools for observing running Godot games — spatial snapshots, deltas, queries, watches, and recordings."
---
```

```yaml
# site/director/index.md
---
description: "Director provides 38+ operations for building Godot scenes, nodes, resources, tilemaps, animations, and shaders via MCP."
---
```

```yaml
# site/examples/index.md
---
description: "Real debugging scenarios with full AI session transcripts — physics tunneling, pathfinding, collision layers, and more."
---
```

```yaml
# site/api/index.md
---
description: "Complete API reference for Theatre's Stage MCP tools — input schemas, response formats, and parameter documentation."
---
```

```yaml
# site/architecture/index.md
---
description: "Theatre's architecture — GDExtension addon, TCP protocol, MCP server, thread model, and security boundaries."
---
```

**Implementation Notes**:
- VitePress automatically uses `description` frontmatter for `<meta name="description">`
- Add to the remaining ~30 content pages following the same pattern
- Each description should be 120-160 characters, unique, and include key terms
- Pages without frontmatter get a `---` block at the top

**Acceptance Criteria**:
- [ ] Every content page has a unique `<meta name="description">` tag
- [ ] No two pages share the same description
- [ ] Descriptions are 120-160 characters

### Unit 2.3: Add dynamic per-page OG tags via transformPageData

**File**: `site/.vitepress/config.mts`

Replace the static OG tags in `head` with dynamic per-page OG generation:

```typescript
import { defineConfig, type HeadConfig } from 'vitepress'

export default defineConfig({
  // ... existing config

  head: [
    ['link', { rel: 'icon', href: '/favicon.svg' }],
    // Remove static og:title, og:description, og:url — they move to transformHead
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'Theatre' }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ['script', { async: '', src: 'https://www.googletagmanager.com/gtag/js?id=G-QDTG6Z9L05' }],
    ['script', {}, "window.dataLayer=window.dataLayer||[];function gtag(){dataLayer.push(arguments)}gtag('js',new Date());gtag('config','G-QDTG6Z9L05')"],
  ],

  transformHead({ pageData }) {
    const head: HeadConfig[] = []
    const title = pageData.title || 'Theatre'
    const description = pageData.description || 'AI agent toolkit for building and debugging Godot games'
    const url = `https://godot-theatre.dev/${pageData.relativePath.replace(/\.md$/, '').replace(/index$/, '')}`

    head.push(['meta', { property: 'og:title', content: title }])
    head.push(['meta', { property: 'og:description', content: description }])
    head.push(['meta', { property: 'og:url', content: url }])
    head.push(['link', { rel: 'canonical', href: url }])

    return head
  },

  // ...
})
```

**Implementation Notes**:
- `transformHead` runs at build time for each page
- Generates per-page `og:title`, `og:description`, `og:url`, and canonical `<link>`
- `pageData.title` comes from the H1 heading or frontmatter `title`
- `pageData.description` comes from frontmatter `description`
- Static OG tags (`og:type`, `og:site_name`, `twitter:card`) stay in `head` since they're the same on every page

**Acceptance Criteria**:
- [ ] Each page has unique `og:title` and `og:description` meta tags
- [ ] Each page has a `<link rel="canonical">` tag with the correct URL
- [ ] `og:url` matches the canonical URL
- [ ] URLs use clean format (no `.html`, no trailing `index`)

---

## Phase 3: Structured Data (Medium — 2-3 hours)

JSON-LD schemas for rich result eligibility.

### Unit 3.1: Add WebSite + SoftwareSourceCode schema to homepage

**File**: `site/.vitepress/config.mts`

Add JSON-LD to the `transformHead` function, conditionally for the homepage:

```typescript
transformHead({ pageData }) {
  const head: HeadConfig[] = []
  // ... existing OG/canonical logic from Unit 2.3

  // Homepage-only structured data
  if (pageData.relativePath === 'index.md') {
    head.push(['script', { type: 'application/ld+json' },
      JSON.stringify({
        "@context": "https://schema.org",
        "@graph": [
          {
            "@type": "WebSite",
            "name": "Theatre",
            "url": "https://godot-theatre.dev",
            "description": "AI agent toolkit for building and debugging Godot games",
            "inLanguage": "en"
          },
          {
            "@type": "SoftwareSourceCode",
            "name": "Theatre",
            "description": "AI agent toolkit that gives coding assistants spatial awareness of running Godot games via MCP",
            "url": "https://godot-theatre.dev",
            "codeRepository": "https://github.com/nklisch/theatre",
            "programmingLanguage": ["Rust", "GDScript"],
            "runtimePlatform": "Godot Engine",
            "license": "https://opensource.org/licenses/MIT",
            "applicationCategory": "DeveloperApplication",
            "operatingSystem": "Windows, macOS, Linux",
            "offers": {
              "@type": "Offer",
              "price": "0",
              "priceCurrency": "USD"
            }
          }
        ]
      })
    ])
  }

  return head
},
```

**Acceptance Criteria**:
- [ ] Homepage HTML contains `<script type="application/ld+json">` with `@graph`
- [ ] Graph includes `WebSite` and `SoftwareSourceCode` types
- [ ] JSON-LD validates at https://validator.schema.org/

### Unit 3.2: Add BreadcrumbList schema to all inner pages

**File**: `site/.vitepress/config.mts`

Add BreadcrumbList generation to `transformHead`, for non-homepage pages:

```typescript
// Inside transformHead, after the OG/canonical logic:

if (pageData.relativePath !== 'index.md') {
  const segments = pageData.relativePath
    .replace(/\.md$/, '')
    .replace(/\/index$/, '')
    .split('/')
    .filter(Boolean)

  const breadcrumbs = [
    { "@type": "ListItem", position: 1, name: "Home", item: "https://godot-theatre.dev/" }
  ]

  let path = ''
  for (let i = 0; i < segments.length; i++) {
    path += `/${segments[i]}`
    const name = segments[i]
      .replace(/-/g, ' ')
      .replace(/\b\w/g, c => c.toUpperCase())
    breadcrumbs.push({
      "@type": "ListItem",
      position: i + 2,
      name: i === segments.length - 1 ? (pageData.title || name) : name,
      item: `https://godot-theatre.dev${path}`
    })
  }

  head.push(['script', { type: 'application/ld+json' },
    JSON.stringify({
      "@context": "https://schema.org",
      "@type": "BreadcrumbList",
      "itemListElement": breadcrumbs
    })
  ])
}
```

**Implementation Notes**:
- Auto-generates breadcrumbs from the URL path
- Uses `pageData.title` for the last segment (more readable than URL slug)
- Intermediate segments use title-cased URL slugs

**Acceptance Criteria**:
- [ ] Inner pages contain BreadcrumbList JSON-LD
- [ ] Breadcrumb positions are sequential starting at 1
- [ ] Last breadcrumb name matches the page title
- [ ] JSON-LD validates at https://validator.schema.org/

---

## Phase 4: Image and Accessibility Fixes (Medium — 30 min)

### Unit 4.1: Add alt text and dimensions to logo

**File**: `site/.vitepress/config.mts`

The logo in themeConfig should include alt text:

```typescript
themeConfig: {
  logo: { src: '/logo.svg', alt: 'Theatre logo' },
  // ...
}
```

**Implementation Notes**:
- VitePress supports object syntax for logo with `src` and `alt` properties
- This adds `alt="Theatre logo"` to the `<img>` tag in the navbar

**Acceptance Criteria**:
- [ ] Logo `<img>` tag has `alt="Theatre logo"`

### Unit 4.2: Add .nojekyll to public directory

**File**: `site/public/.nojekyll`

This file already exists according to exploration. Verify it's present.

**Acceptance Criteria**:
- [ ] `.nojekyll` exists in `site/public/`

---

## Phase 5: Content Enhancements (Low priority — ongoing)

These are content-level improvements that don't require code changes. They should be done incrementally.

### Unit 5.1: Add description frontmatter to remaining ~30 pages

Apply the pattern from Unit 2.2 to all remaining content pages. Each page in:
- `site/stage/` (13 pages)
- `site/director/` (12 pages)
- `site/examples/` (8 pages)
- `site/api/` (4 pages — index already done)
- `site/architecture/` (4 pages — index already done)
- `site/changelog.md`

**Template for each page:**
```yaml
---
description: "<120-160 char unique description with key terms>"
---
```

### Unit 5.2: Add a self-contained definition to getting-started.md

Add a one-line Theatre definition at the top of `/guide/getting-started.md` so the page is citable in isolation:

```markdown
# Getting Started

[Theatre](/) gives AI agents spatial awareness of running Godot games via MCP tools. This guide walks you through getting your first `spatial_snapshot` in under 10 minutes.
```

### Unit 5.3: Expand examples/index.md (borderline thin at ~300 words)

Add a summary table and brief methodology description to push the page above 500 words.

---

## Implementation Order

1. **Phase 1** (Units 1.1-1.5): Crawl infrastructure — do all in one commit
2. **Phase 2** (Units 2.1-2.3): On-page meta — do all in one commit
3. **Phase 3** (Units 3.1-3.2): Structured data — do all in one commit
4. **Phase 4** (Unit 4.1): Image accessibility — can combine with Phase 3 commit
5. **Phase 5** (Units 5.1-5.3): Content enhancements — individual commits as needed

Phases 1-3 are the highest-impact changes. Phase 1 alone would move the Technical SEO score from 32 to ~60. Phases 1-3 combined would move the overall Health Score from **42 to ~65-70**.

---

## Testing

### Build Verification: `site/.vitepress/dist/`

After `npm run build` in `site/`:

```bash
# Verify sitemap exists and has content
test -f .vitepress/dist/sitemap.xml && echo "PASS: sitemap exists"
grep -c "<url>" .vitepress/dist/sitemap.xml  # Should be ~45

# Verify robots.txt
test -f .vitepress/dist/robots.txt && echo "PASS: robots.txt exists"
grep "Sitemap:" .vitepress/dist/robots.txt

# Verify llms.txt
test -f .vitepress/dist/llms.txt && echo "PASS: llms.txt exists"

# Verify clean URLs (no .html in sitemap)
grep -c "\.html" .vitepress/dist/sitemap.xml  # Should be 0

# Verify canonical tags in built HTML
grep -l 'rel="canonical"' .vitepress/dist/guide/getting-started.html

# Verify OG tags are per-page (not all identical)
grep 'og:title' .vitepress/dist/index.html
grep 'og:title' .vitepress/dist/guide/getting-started.html
# These should show different values

# Verify JSON-LD on homepage
grep 'application/ld+json' .vitepress/dist/index.html

# Verify BreadcrumbList on inner page
grep 'BreadcrumbList' .vitepress/dist/guide/getting-started.html

# Verify logo alt text
grep 'alt="Theatre logo"' .vitepress/dist/index.html
```

### Preview Verification

```bash
cd site && npm run build && npm run preview
# Then manually check:
# - https://localhost:4173/ → correct title, OG tags, JSON-LD
# - https://localhost:4173/guide/getting-started → canonical, breadcrumbs, description
# - https://localhost:4173/robots.txt → serves correctly
# - https://localhost:4173/sitemap.xml → valid XML
# - https://localhost:4173/llms.txt → serves correctly
```

## Verification Checklist

```bash
cd /home/nathan/dev/theatre/site
npm run build
# Phase 1
test -f .vitepress/dist/sitemap.xml
test -f .vitepress/dist/robots.txt
test -f .vitepress/dist/llms.txt
# Phase 2
grep 'rel="canonical"' .vitepress/dist/guide/getting-started.html
grep 'og:title.*Getting Started' .vitepress/dist/guide/getting-started.html
# Phase 3
grep 'application/ld+json' .vitepress/dist/index.html
grep 'BreadcrumbList' .vitepress/dist/guide/getting-started.html
# Phase 4
grep 'alt="Theatre logo"' .vitepress/dist/index.html
```

## Projected Score Impact

| Category | Current | After Phase 1 | After Phase 1-3 |
|----------|---------|---------------|------------------|
| Technical SEO | 32 | ~60 | ~75 |
| Content Quality | 68 | 70 | 72 |
| On-Page SEO | 35 | 40 | ~70 |
| Schema / Structured Data | 5 | 5 | ~65 |
| Performance (CWV) | 78 | 78 | 78 |
| AI Search Readiness | 52 | ~65 | ~72 |
| Images | 35 | 35 | ~45 |
| **Overall** | **42** | **~55** | **~70** |
