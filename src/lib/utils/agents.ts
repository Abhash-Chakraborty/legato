const NAMES: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  codebuddy: "CodeBuddy",
};

/** An agent's name as people say it, from Inyon's kind ("claude"). */
export const agentName = (agent: string) =>
  NAMES[agent] ?? agent.charAt(0).toUpperCase() + agent.slice(1);
