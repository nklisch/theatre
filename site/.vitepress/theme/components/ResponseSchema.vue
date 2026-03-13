<script setup lang="ts">
defineProps<{
  schema?: {
    type: string
    properties: Record<string, any>
    required?: string[]
  } | null
}>()

function formatType(prop: any): string {
  if (prop.type === 'array' && prop.items) {
    return `${prop.items.type ?? 'object'}[]`
  }
  return prop.type ?? 'any'
}
</script>

<template>
  <div v-if="schema && schema.properties">
    <table class="param-table">
      <thead>
        <tr>
          <th>Field</th>
          <th>Type</th>
          <th>Description</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(prop, name) in schema.properties" :key="name">
          <td><code>{{ name }}</code></td>
          <td><code>{{ formatType(prop) }}</code></td>
          <td>{{ prop.description ?? '' }}</td>
        </tr>
      </tbody>
    </table>
  </div>
  <div v-else class="no-schema">
    <em>Response schema not available for this tool.</em>
  </div>
</template>

<style scoped>
.param-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9em;
}
.param-table th {
  text-align: left;
  border-bottom: 2px solid var(--vp-c-divider);
  padding: 8px 12px;
}
.param-table td {
  border-bottom: 1px solid var(--vp-c-divider);
  padding: 8px 12px;
  vertical-align: top;
}
.no-schema {
  color: var(--vp-c-text-2);
  font-style: italic;
}
</style>
