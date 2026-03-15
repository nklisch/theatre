import { defineConfig, type HeadConfig } from 'vitepress'

export default defineConfig({
	base: '/',
	title: 'Theatre',
	titleTemplate: ':title — AI Toolkit for Godot',
	description: 'AI agent toolkit for building and debugging Godot games',
	sitemap: {
		hostname: 'https://godot-theatre.dev',
	},
	cleanUrls: true,
	lastUpdated: true,
	head: [
		['link', { rel: 'icon', href: '/favicon.svg' }],
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

		// BreadcrumbList for all inner pages
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
					.replace(/\b\w/g, (c: string) => c.toUpperCase())
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

		return head
	},

	// Custom domain — CNAME file placed in site/public/CNAME

	themeConfig: {
		logo: { src: '/logo.svg', alt: 'Theatre logo' },
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
