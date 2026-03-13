import type { Theme } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import './custom.css'
import ToolCard from './components/ToolCard.vue'
import ScenarioCard from './components/ScenarioCard.vue'
import AgentConversation from './components/AgentConversation.vue'
import ArchDiagram from './components/ArchDiagram.vue'
import HeroSection from './components/HeroSection.vue'
import ParamTable from './components/ParamTable.vue'
import ResponseSchema from './components/ResponseSchema.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('ToolCard', ToolCard)
    app.component('ScenarioCard', ScenarioCard)
    app.component('AgentConversation', AgentConversation)
    app.component('ArchDiagram', ArchDiagram)
    app.component('HeroSection', HeroSection)
    app.component('ParamTable', ParamTable)
    app.component('ResponseSchema', ResponseSchema)
  }
} satisfies Theme
