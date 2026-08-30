import { agentApi, sessionApi, type AgentDto, type SessionDto } from '$lib/api';

class AgentStore {
  agents = $state<AgentDto[]>([]);
  sessions = $state<SessionDto[]>([]);
  sessionsByAgent = $state<Record<string, SessionDto[]>>({});
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
      const sessions = await sessionApi.list(agentId);
      this.sessions = sessions;
      if (agentId) {
        this.sessionsByAgent = {
          ...this.sessionsByAgent,
          [agentId]: sessions,
        };
      }
    } catch (e) {
      console.error('Failed to load sessions:', e);
    }
  }

  async loadAllSessions() {
    try {
      const sessions = await sessionApi.list();
      this.sessions = sessions;
      this.sessionsByAgent = sessions.reduce<Record<string, SessionDto[]>>((grouped, session) => {
        grouped[session.agent_id] = [...(grouped[session.agent_id] ?? []), session];
        return grouped;
      }, {});
    } catch (e) {
      console.error('Failed to load all sessions:', e);
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

  private insertSession(session: SessionDto) {
    const nextByAgent = {
      ...this.sessionsByAgent,
      [session.agent_id]: [session, ...(this.sessionsByAgent[session.agent_id] ?? [])],
    };
    this.sessionsByAgent = nextByAgent;
    this.sessions = [session, ...this.sessions];
  }

  async createSession(agentId: string, title?: string) {
    try {
      const session = await sessionApi.create(agentId, title);
      this.insertSession(session);
      this.currentSession = session;
      return session;
    } catch (e) {
      console.error('Failed to create session:', e);
      throw e;
    }
  }

  async deleteSession(id: string) {
    const session = this.sessions.find((item) => item.id === id);
    await sessionApi.delete(id);
    this.sessions = this.sessions.filter((item) => item.id !== id);
    if (session) {
      this.sessionsByAgent = {
        ...this.sessionsByAgent,
        [session.agent_id]: (this.sessionsByAgent[session.agent_id] ?? []).filter(
          (item) => item.id !== id,
        ),
      };
    }
    if (this.currentSession?.id === id) {
      this.currentSession = null;
    }
  }

  selectAgent(agent: AgentDto) {
    this.currentAgent = agent;
    if (this.currentSession?.agent_id !== agent.id) {
      this.currentSession = null;
    }
    if (!this.sessionsByAgent[agent.id]) {
      void this.loadSessions(agent.id);
    }
  }

  selectSession(session: SessionDto) {
    this.currentSession = session;
  }
}

export const agentStore = new AgentStore();
