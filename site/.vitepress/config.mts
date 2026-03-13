import { defineConfig } from 'vitepress'

export default defineConfig({
  base: '/',
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

  // Custom domain — CNAME file placed in site/public/CNAME

  themeConfig: {
    logo: '/logo.svg',
    siteTitle: 'Theatre',

    nav: [
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'Spectator', link: '/spectator/' },
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

      '/spectator/': [
        {
          text: 'Spectator',
          items: [
            { text: 'Overview', link: '/spectator/' },
            { text: 'Spatial Snapshot', link: '/spectator/snapshot' },
            { text: 'Spatial Delta', link: '/spectator/delta' },
            { text: 'Spatial Query', link: '/spectator/query' },
            { text: 'Spatial Inspect', link: '/spectator/inspect' },
            { text: 'Spatial Watch', link: '/spectator/watch' },
            { text: 'Spatial Config', link: '/spectator/config' },
            { text: 'Spatial Action', link: '/spectator/action' },
            { text: 'Scene Tree', link: '/spectator/scene-tree' },
            { text: 'Recording', link: '/spectator/recording' },
          ]
        },
        {
          text: 'Workflows',
          items: [
            { text: 'The Dashcam', link: '/spectator/dashcam' },
            { text: 'Watch & React', link: '/spectator/watch-workflow' },
            { text: 'Editor Dock', link: '/spectator/editor-dock' },
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
            { text: 'Director + Spectator Loop', link: '/examples/build-verify' },
          ]
        }
      ],

      '/api/': [
        {
          text: 'API Reference',
          items: [
            { text: 'Spectator Tools', link: '/api/' },
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
      { icon: 'github', link: 'https://github.com/nklisch/theatre' }
    ],

    search: {
      provider: 'local'
    },

    footer: {
      message: 'Open source under the MIT License.',
      copyright: 'Theatre — AI toolkit for Godot'
    },

    editLink: {
      pattern: 'https://github.com/nklisch/theatre/edit/main/site/:path',
      text: 'Edit this page on GitHub'
    }
  }
})
