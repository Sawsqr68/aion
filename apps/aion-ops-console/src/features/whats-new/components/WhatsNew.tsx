import { createConfiguredApiClient } from '@/lib/config';

import { WhatsNewPanel } from './WhatsNewPanel';

/**
 * The shell-facing What's-new entry point: {@link WhatsNewPanel} bound to the
 * configured API client. Namespace-independent (like `/whoami`), so one
 * client built once outside render is correct for the app's lifetime.
 */
const fetchChangelog = () => createConfiguredApiClient().getChangelog();

export function WhatsNew() {
  return <WhatsNewPanel fetchChangelog={fetchChangelog} />;
}
