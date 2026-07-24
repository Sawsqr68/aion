import { useEffect, useState } from 'react';
import { Handle, type NodeProps, Position } from 'reactflow';

import type { ProjectionStep } from '../lib/projection-types';
import { StepNodeBody } from './StepNodeBody';

export type CanvasNodeData = {
  kind: 'step';
  name: string;
  label: string;
  step?: ProjectionStep;
  expandedSubflows?: ReadonlySet<string>;
  onToggleSubflow?: ((path: string) => void) | undefined;
  onJumpTo?: ((byteOffset: number) => void) | undefined;
  diagnostic?: 'primary' | 'cascade' | undefined;
  onActivate: () => void;
  proseEditing?: boolean;
  onBeginProseEdit?: (() => void) | undefined;
  onCancelProseEdit?: (() => void) | undefined;
  onSaveProse?: ((prose: string) => Promise<void>) | undefined;
};

export function StepCanvasNode({ data, selected }: NodeProps<CanvasNodeData>) {
  const [draft, setDraft] = useState(data.label);
  const [saving, setSaving] = useState(false);
  useEffect(() => setDraft(data.label), [data.label]);

  const saveProse = async () => {
    if (data.onSaveProse === undefined) return;
    setSaving(true);
    try {
      await data.onSaveProse(draft);
    } finally {
      setSaving(false);
    }
  };
  return (
    <div>
      <Handle
        className="!size-2 !border-surface-elevated !bg-muted-foreground"
        position={Position.Top}
        type="target"
      />
      {data.proseEditing === true ? (
        <article
          aria-label={`Step ${data.name}`}
          className="min-w-52 max-w-64 rounded-xl border border-accent-primary bg-surface-elevated shadow-md"
        >
          <form
            aria-label={`Edit prose for ${data.name}`}
            className="nodrag p-3"
            onSubmit={(event) => {
              event.preventDefault();
              void saveProse();
            }}
          >
            <label className="block text-muted-foreground text-xs">
              Step prose
              <textarea
                className="mt-2 min-h-20 w-full resize-y rounded-md border border-border bg-surface-base p-2 text-foreground text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent-primary"
                onChange={(event) => setDraft(event.target.value)}
                value={draft}
              />
            </label>
            <span className="mt-2 flex justify-end gap-2">
              <button
                className="rounded-md px-2 py-1 text-muted-foreground text-xs hover:bg-accent"
                onClick={data.onCancelProseEdit}
                type="button"
              >
                Cancel
              </button>
              <button
                className="rounded-md bg-primary px-2 py-1 text-primary-foreground text-xs disabled:opacity-50"
                disabled={saving}
                type="submit"
              >
                {saving ? 'Saving…' : 'Save prose'}
              </button>
            </span>
          </form>
        </article>
      ) : data.step !== undefined ? (
        <>
          <StepNodeBody
            diagnostic={data.diagnostic}
            expanded={data.expandedSubflows ?? new Set()}
            onJumpTo={data.onJumpTo ?? (() => data.onActivate())}
            onToggleSubflow={data.onToggleSubflow ?? (() => undefined)}
            path={data.step.name}
            selected={selected}
            step={data.step}
          />
          {data.onBeginProseEdit !== undefined && (
            <button
              aria-label={`Edit prose for ${data.name}`}
              className="nodrag mt-1 rounded-md px-2 py-1 text-muted-foreground text-xs outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-accent-primary"
              onClick={data.onBeginProseEdit}
              type="button"
            >
              Edit prose
            </button>
          )}
        </>
      ) : null}
      <Handle
        className="!size-2 !border-surface-elevated !bg-muted-foreground"
        position={Position.Bottom}
        type="source"
      />
    </div>
  );
}
