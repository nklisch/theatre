<script setup lang="ts">
defineProps<{
  params: {
    name: string
    type: string
    required: boolean
    description: string
    default?: string
    enum?: string[]
  }[]
}>()
</script>

<template>
  <div v-if="params && params.length > 0">
    <table class="param-table">
      <thead>
        <tr>
          <th>Parameter</th>
          <th>Type</th>
          <th>Required</th>
          <th>Description</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="p in params" :key="p.name">
          <td>
            <code>{{ p.name }}</code>
          </td>
          <td>
            <code>{{ p.type }}</code>
            <span v-if="p.enum" class="enum-values">
              <br />
              <small>{{ p.enum.map(v => `"${v}"`).join(' | ') }}</small>
            </span>
          </td>
          <td>
            <span v-if="p.required" class="badge required">required</span>
            <span v-else class="badge optional">
              optional
              <template v-if="p.default">
                <br /><small>default: <code>{{ p.default }}</code></small>
              </template>
            </span>
          </td>
          <td>{{ p.description }}</td>
        </tr>
      </tbody>
    </table>
  </div>
  <div v-else class="no-params">
    <em>No parameters.</em>
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
.badge {
  display: inline-block;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.8em;
  font-weight: 600;
}
.required {
  background: var(--vp-c-danger-soft);
  color: var(--vp-c-danger-1);
}
.optional {
  background: var(--vp-c-default-soft);
  color: var(--vp-c-text-2);
}
.enum-values {
  color: var(--vp-c-text-3);
}
.no-params {
  color: var(--vp-c-text-2);
  font-style: italic;
}
</style>
