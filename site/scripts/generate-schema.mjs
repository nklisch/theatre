// Cross-platform schema generation; the site already requires Node.js.
import { spawnSync } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const site = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const result = spawnSync('cargo', ['run', '-p', 'theatre-docs-gen', '--quiet'], {
  cwd: resolve(site, '..'),
  encoding: 'utf8',
  maxBuffer: 16 * 1024 * 1024,
  windowsHide: true,
  stdio: ['ignore', 'pipe', 'inherit'],
})
if (result.error) throw result.error
if (result.status !== 0) process.exit(result.status ?? 1)
const tools = JSON.parse(result.stdout)
if (!Array.isArray(tools)) throw new Error('Schema generator did not return a tool array')
mkdirSync(resolve(site, '.generated'), { recursive: true })
writeFileSync(resolve(site, '.generated/tools.json'), `${JSON.stringify(tools, null, 2)}\n`)
console.log(`Generated schemas for ${tools.length} tools in site/.generated/tools.json`)
