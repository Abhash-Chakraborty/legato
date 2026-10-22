/** An agent's name as people say it, from Inyon's kind ("claude"). */
export const agentName = (agent: string) =>
  ({ claude: "Claude", codex: "Codex", gemini: "Gemini" })[agent] ??
  agent.charAt(0).toUpperCase() + agent.slice(1);
