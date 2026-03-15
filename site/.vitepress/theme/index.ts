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
import FlowDiagram from './components/FlowDiagram.vue'
import SequenceDiagram from './components/SequenceDiagram.vue'
import FrameDiagram from './components/FrameDiagram.vue'
import DepGraph from './components/DepGraph.vue'

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
    app.component('FlowDiagram', FlowDiagram)
    app.component('SequenceDiagram', SequenceDiagram)
    app.component('FrameDiagram', FrameDiagram)
    app.component('DepGraph', DepGraph)
  }
} satisfies Theme
