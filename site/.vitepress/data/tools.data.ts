import { readFileSync, existsSync } from 'fs'
import { resolve } from 'path'

interface ToolParam {
  name: string
  type: string
  required: boolean
  description: string
  default?: string
  enum?: string[]
}

interface ToolDoc {
  server: 'spectator' | 'director'
  name: string
  description: string
  input_schema: {
    type: string
    properties: Record<string, any>
    required?: string[]
    $defs?: Record<string, any>
    definitions?: Record<string, any>
  }
  output_schema?: {
    type: string
    properties: Record<string, any>
    required?: string[]
  }
}

/**
 * Resolve a $ref against the definitions dict.
 * Handles both #/$defs/ (JSON Schema 2020-12) and #/definitions/ (older drafts).
 */
function resolveRef(ref: string, defs: Record<string, any>): any | undefined {
  const defs_prefix = '#/$defs/'
  const definitions_prefix = '#/definitions/'
  if (ref.startsWith(defs_prefix)) {
    return defs[ref.slice(defs_prefix.length)]
  }
  if (ref.startsWith(definitions_prefix)) {
    return defs[ref.slice(definitions_prefix.length)]
  }
  return undefined
}

/**
 * Extract enum values from a schema definition.
 * Handles both plain `enum` arrays and `oneOf` with `const` values
 * (used for enums-with-descriptions like PerspectiveMode, WatchAction).
 */
function extractEnumValues(def: any): string[] | undefined {
  if (def.enum) {
    return def.enum as string[]
  }
  if (def.oneOf && Array.isArray(def.oneOf)) {
    const consts = def.oneOf
      .filter((s: any) => s.const !== undefined)
      .map((s: any) => String(s.const))
    if (consts.length > 0) return consts
  }
  return undefined
}

/**
 * Flatten a JSON Schema property into a ToolParam.
 * Handles $ref resolution, oneOf/anyOf, nested objects.
 */
function schemaToParam(
  name: string,
  prop: any,
  required: string[],
  defs: Record<string, any>
): ToolParam {
  let resolved = prop
  let enumValues: string[] | undefined

  // Resolve top-level $ref
  if (prop.$ref) {
    const def = resolveRef(prop.$ref, defs)
    if (def) {
      resolved = { ...def, ...prop, $ref: undefined }
      enumValues = extractEnumValues(def)
    }
  }

  let type = resolved.type ?? 'any'

  // Handle anyOf (typically nullable: non-null variant + null)
  if (resolved.anyOf) {
    const nonNull = resolved.anyOf.filter((s: any) => s.type !== 'null')
    if (nonNull.length === 1) {
      const inner = nonNull[0]
      if (inner.$ref) {
        const def = resolveRef(inner.$ref, defs)
        if (def) {
          enumValues = extractEnumValues(def)
          type = enumValues ? 'string' : (def.type ?? inner.$ref.split('/').pop() ?? 'any')
        } else {
          type = inner.$ref.split('/').pop() ?? 'any'
        }
      } else {
        type = inner.type ?? 'any'
        if (inner.enum) enumValues = inner.enum
        if (!enumValues) enumValues = extractEnumValues(inner)
      }
    }
  }

  // Handle nullable field (not standard JSON Schema but used in some generators)
  if (resolved.nullable && type === 'any' && resolved.type) {
    type = resolved.type
  }

  // Inline enum on the resolved schema (after $ref resolution)
  if (!enumValues && resolved.enum) enumValues = resolved.enum
  if (!enumValues) enumValues = extractEnumValues(resolved)

  // Array types
  if (type === 'array' && resolved.items) {
    const items = resolved.items
    let itemType: string
    if (items.$ref) {
      const def = resolveRef(items.$ref, defs)
      itemType = def?.type ?? items.$ref.split('/').pop() ?? 'any'
    } else {
      itemType = items.type ?? 'any'
    }
    type = `${itemType}[]`
  }

  // integer → number for display
  if (type === 'integer') type = 'number'

  const param: ToolParam = {
    name,
    type,
    required: required.includes(name),
    description: resolved.description ?? prop.description ?? '',
  }

  if (resolved.default !== undefined) {
    param.default = JSON.stringify(resolved.default)
  } else if (prop.default !== undefined) {
    param.default = JSON.stringify(prop.default)
  }

  if (enumValues && enumValues.length > 0) {
    param.enum = enumValues
  }

  return param
}

function parseToolParams(tool: ToolDoc): ToolParam[] {
  const schema = tool.input_schema
  if (!schema.properties) return []

  const required = schema.required ?? []
  // Support both $defs (2020-12) and definitions (older)
  const defs: Record<string, any> = schema.$defs ?? schema.definitions ?? {}

  return Object.entries(schema.properties)
    .map(([name, prop]) => schemaToParam(name, prop, required, defs))
}

export default {
  watch: ['../../.generated/tools.json'],
  load(): { tools: ToolDoc[]; params: Record<string, ToolParam[]> } {
    const jsonPath = resolve(__dirname, '../../.generated/tools.json')

    if (!existsSync(jsonPath)) {
      console.warn('tools.json not found — run site/scripts/generate-schema.sh first')
      return { tools: [], params: {} }
    }

    const tools: ToolDoc[] = JSON.parse(readFileSync(jsonPath, 'utf-8'))
    const params: Record<string, ToolParam[]> = {}

    for (const tool of tools) {
      params[tool.name] = parseToolParams(tool)
    }

    return { tools, params }
  }
}
