<script setup lang="ts">
/**
 * Renders a simulated human-agent-tool conversation.
 *
 * Usage in markdown:
 * <AgentConversation :messages="[
 *   { role: 'human', text: 'My character clips through the east wall.' },
 *   { role: 'agent', text: 'Let me check what\'s near that wall...' },
 *   { role: 'tool', text: 'spatial_query { type: \"radius\", origin: \"player\", radius: 5 }' },
 *   { role: 'agent', text: 'I can see the wall\'s collision shape...' },
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
