import { agentApi, sessionApi, type AgentDto, type SessionDto } from '$lib/api';

class AgentStore {
  agents = $state<AgentDto[]>([]);
  sessions = $state<SessionDto[]>([]);
  currentAgent = $state<AgentDto | null>(null);
  currentSession = $state<SessionDto | null>(null);
  loading = $state(false);

  async loadAgents() {
    this.loading = true;
    try {
      this.agents = await agentApi.list();
    } catch (e) {
      console.error('Failed to load agents:', e);
    } finally {
      this.loading = false;
    }
  }

  async loadSessions(agentId?: string) {
    try {
      this.sessions = await sessionApi.list(agentId);
    } catch (e) {
      console.error('Failed to load sessions:', e);
    }
  }

  async createAgent(name: string, description?: string, systemPrompt?: string) {
    const agent = await agentApi.create(name, description, systemPrompt);
    this.agents = [...this.agents, agent];
    return agent;
  }

  async deleteAgent(id: string) {
    await agentApi.delete(id);
    this.agents = this.agents.filter((a) => a.id !== id);
    if (this.currentAgent?.id === id) {
      this.currentAgent = null;
    }
  }

  async createSession(agentId: string, title?: string) {
    try {
      console.log('Creating session:', { agentId, title });
      const session = await sessionApi.create(agentId, title);
      console.log('Session created:', session);
      this.sessions = [session, ...this.sessions];
      this.currentSession = session;
      return session;
    } catch (e) {
      console.error('Failed to create session:', e);
      throw e;
    }
  }

  async deleteSession(id: string) {
    await sessionApi.delete(id);
    this.sessions = this.sessions.filter((s) => s.id !== id);
    if (this.currentSession?.id === id) {
      this.currentSession = null;
    }
  }

  selectAgent(agent: AgentDto) {
    this.currentAgent = agent;
    this.loadSessions(agent.id);
  }

  selectSession(session: SessionDto) {
    this.currentSession = session;
  }
}

export const agentStore = new AgentStore();
