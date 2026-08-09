import {
  AlignLeft,
  Check,
  CloudUpload,
  Columns2,
  FileText,
  HardHat,
  LayoutDashboard,
  LoaderCircle,
  Rocket,
  Save,
  Workflow,
} from 'lucide-react';
import { useId } from 'react';

import { Badge } from '@/components/ui';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

import type { DeployEligibility } from '../lib/deploy-eligibility';
import type { AuthoringViewMode } from './ProjectionPane';
import { RunPanel, type RunPanelProps } from './RunPanel';

/** Which header action is in flight; the whole row locks while one is. */
export type EditorWorking = 'format' | 'save' | 'deploy' | null;

export type EditorHeaderProps = {
  /** Buffer differs from the last save. */
  dirty: boolean;
  /** Whether Deploy is offered, and the reason when it is withheld. */
  deploy: DeployEligibility;
  onDeploy: () => void;
  onFormat: () => void;
  onSave: () => void;
  onSelectViewMode: (mode: AuthoringViewMode) => void;
  path: string;
  /** Everything the run panel needs; the header only relays it. */
  run: RunPanelProps;
  viewMode: AuthoringViewMode;
  workerDocument: boolean;
  working: EditorWorking;
};

/**
 * Document identity, the view-mode switch, and the document actions:
 * Format → Save → Deploy → Run.
 *
 * Deploy and Run are SEPARATE buttons by operator ruling: Deploy ships the
 * saved buffer and never starts a run; Run opens the start panel and never
 * deploys. No chain between them. Extracted from `EditorPane` with an
 * explicit props contract so the whole action row renders (and is tested)
 * without CodeMirror behind it.
 */
export function EditorHeader({
  dirty,
  deploy,
  onDeploy,
  onFormat,
  onSave,
  onSelectViewMode,
  path,
  run,
  viewMode,
  workerDocument,
  working,
}: EditorHeaderProps) {
  const documentName = path.split('/').at(-1) ?? path;
  return (
    <header className="flex flex-wrap items-center justify-between gap-3 border-border border-b bg-surface-base px-4 py-3">
      <div className="flex min-w-0 items-center gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h2 className="truncate font-semibold text-foreground text-sm">{documentName}</h2>
            <Badge className="shrink-0" variant={dirty ? 'secondary' : 'outline'}>
              {dirty ? 'Modified' : 'Saved'}
            </Badge>
          </div>
          <p className="mt-0.5 truncate font-mono text-muted-foreground text-[0.6875rem]">{path}</p>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <fieldset
          className="flex rounded-lg border border-border bg-surface-elevated p-0.5"
          aria-label="Authoring view"
        >
          <ViewButton
            icon={<FileText className="size-3.5" />}
            label="Text"
            mode="text"
            selected={viewMode}
            onSelect={onSelectViewMode}
          />
          <ViewButton
            icon={<Workflow className="size-3.5" />}
            label="Canvas"
            mode="canvas"
            selected={viewMode}
            onSelect={onSelectViewMode}
          />
          <ViewButton
            icon={<Columns2 className="size-3.5" />}
            label="Split"
            mode="split"
            selected={viewMode}
            onSelect={onSelectViewMode}
          />
          <ViewButton
            icon={<LayoutDashboard className="size-3.5" />}
            label="Studio"
            mode="studio"
            selected={viewMode}
            onSelect={onSelectViewMode}
          />
          <ViewButton
            icon={
              workerDocument ? <HardHat className="size-3.5" /> : <Rocket className="size-3.5" />
            }
            label={workerDocument ? 'Deploy worker' : 'Ship & Run'}
            mode="run"
            selected={viewMode}
            onSelect={onSelectViewMode}
          />
        </fieldset>
        <Button disabled={working !== null} onClick={onFormat} size="sm" variant="outline">
          <ActionIcon action="format" dirty={dirty} working={working} />
          {working === 'format' ? 'Formatting…' : 'Format'}
        </Button>
        <Button disabled={!dirty || working !== null} onClick={onSave} size="sm">
          <ActionIcon action="save" dirty={dirty} working={working} />
          {working === 'save' ? 'Saving…' : 'Save'}
        </Button>
        <DeployButton deploy={deploy} dirty={dirty} onDeploy={onDeploy} working={working} />
        {/* Run is offered for every non-worker document — running needs no
            deploy from THIS session; it targets whatever the server holds. */}
        {!workerDocument && <RunPanel {...run} />}
      </div>
    </header>
  );
}

/**
 * The Deploy affordance, in whichever state eligibility ruled. Withheld
 * states use `aria-disabled` instead of `disabled`: the button stays
 * focusable, so a keyboard/AT operator can reach it and hear WHY it is
 * withheld — `aria-describedby` points at the reason rendered as visible
 * text beside it — and the click no-op is the actual disabling.
 */
function DeployButton({
  deploy,
  dirty,
  onDeploy,
  working,
}: {
  deploy: DeployEligibility;
  dirty: boolean;
  onDeploy: () => void;
  working: EditorWorking;
}) {
  const reasonId = useId();
  if (deploy.kind === 'hidden') return null;
  const blocked = deploy.kind === 'disabled' || working !== null;
  return (
    <>
      <Button
        aria-describedby={deploy.kind === 'disabled' ? reasonId : undefined}
        aria-disabled={blocked ? true : undefined}
        className={cn(blocked && 'opacity-50')}
        onClick={() => {
          if (!blocked) onDeploy();
        }}
        size="sm"
        title={deploy.kind === 'disabled' ? deploy.reason : undefined}
        variant="outline"
      >
        <ActionIcon action="deploy" dirty={dirty} working={working} />
        {working === 'deploy' ? 'Deploying…' : 'Deploy'}
      </Button>
      {deploy.kind === 'disabled' && (
        <span
          className="max-w-56 text-muted-foreground text-xs"
          data-slot="deploy-reason"
          id={reasonId}
        >
          {deploy.reason}
        </span>
      )}
    </>
  );
}

function ViewButton({
  icon,
  label,
  mode,
  selected,
  onSelect,
}: {
  icon: React.ReactNode;
  label: string;
  mode: AuthoringViewMode;
  selected: AuthoringViewMode;
  onSelect: (mode: AuthoringViewMode) => void;
}) {
  return (
    <button
      aria-label={`${label} view`}
      aria-pressed={selected === mode}
      className={cn(
        'flex items-center gap-1.5 rounded-md px-2 py-1 text-xs outline-none focus-visible:ring-2 focus-visible:ring-accent-primary',
        selected === mode
          ? 'bg-accent text-foreground'
          : 'text-muted-foreground hover:text-foreground'
      )}
      onClick={() => onSelect(mode)}
      type="button"
    >
      {icon}
      <span className="hidden lg:inline">{label}</span>
    </button>
  );
}

function ActionIcon({
  action,
  dirty,
  working,
}: {
  action: Exclude<EditorWorking, null>;
  dirty: boolean;
  working: EditorWorking;
}) {
  if (working === action) {
    return <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />;
  }
  if (action === 'format') return <AlignLeft aria-hidden="true" className="size-3.5" />;
  if (action === 'deploy') return <CloudUpload aria-hidden="true" className="size-3.5" />;
  if (dirty) return <Save aria-hidden="true" className="size-3.5" />;
  return <Check aria-hidden="true" className="size-3.5" />;
}
