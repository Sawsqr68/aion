import { useNavigate } from 'react-router';

import { ACTION_IDS, useAction } from '@/lib/keybindings';

import { assistantPath, namespacesPath, workflowListPath } from './routePaths';

/**
 * Registers the shell's global action handlers with the central keybinding
 * registry: view jumps (g w / g n / g a). Mounted once by the always-present
 * AppShell so the actions are live app-wide; the palette open action is
 * registered by its own host. Renders nothing.
 */
export function GlobalActions() {
  const navigate = useNavigate();

  useAction(ACTION_IDS.goWorkflows, () => navigate(workflowListPath));
  useAction(ACTION_IDS.goNamespaces, () => navigate(namespacesPath));
  useAction(ACTION_IDS.goAssistant, () => navigate(assistantPath));

  return null;
}
