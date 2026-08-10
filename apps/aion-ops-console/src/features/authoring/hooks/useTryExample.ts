import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useState } from 'react';
import { useSearchParams } from 'react-router';

import { authoringFacade } from '../lib/facade';
import { paramsForCreatedDocument, runTryExample } from '../lib/try-example';
import { fetchAuthoringDocuments } from './useAuthoringDocuments';

/**
 * The guide's "Try it" behaviour behind one example: thin wiring of the pure
 * `runTryExample` flow (create verbatim → refresh the list → navigate, in
 * that ruled order — the ordering pin lives with the flow) to the real
 * facade, query cache, and URL.
 *
 * `tryExample` never rejects — any refusal (the server's message verbatim,
 * or a list refresh that failed after the create) settles into `refusal` for
 * the card to announce. `pending` covers the whole flow so the button cannot
 * fire a second create while the first is in flight.
 */
export function useTryExample(example: string) {
  const [, setSearchParams] = useSearchParams();
  const queryClient = useQueryClient();
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);

  const tryExample = useCallback(async () => {
    setPending(true);
    setRefusal(null);
    try {
      await runTryExample(
        {
          create: (name) => authoringFacade.createDocument(name, example),
          refreshDocuments: async () => {
            await fetchAuthoringDocuments(queryClient);
          },
          navigate: (path) =>
            setSearchParams((previous) => paramsForCreatedDocument(previous, path)),
        },
        example
      );
    } catch (error) {
      setRefusal(error instanceof Error ? error.message : String(error));
    } finally {
      setPending(false);
    }
  }, [example, queryClient, setSearchParams]);

  return { pending, refusal, tryExample };
}
