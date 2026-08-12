import { type AuthOptions, authHeaders } from "./auth.js";
import {
  AlreadyExistsError,
  InvalidArgumentError,
  QueryTimeoutError,
  mapHttpResponseError,
  mapTransportError,
  mapWireError,
} from "./errors.js";
import { WorkflowHandle } from "./handle.js";
import {
  type Payload,
  type WireEnvelope,
  fromPayload,
  toPayload,
} from "./payload.js";
import type { SubscribeTransport } from "./stream.js";
import { WebSocketSubscribeTransport } from "./ws.js";

export interface TlsOptions {
  readonly enabled?: boolean;
  readonly caCertificate?: string;
}

export interface ClientOptions {
  readonly endpoint: string;
  readonly auth?: AuthOptions;
  readonly tls?: TlsOptions;
  readonly namespace?: string;
  readonly transport?: WorkflowTransport;
  readonly streamTransport?: SubscribeTransport;
  readonly fetch?: typeof fetch;
}

export interface WorkflowRef {
  readonly workflowId: string;
  readonly runId?: string;
}

export interface StartOptions<I> {
  readonly namespace?: string;
  readonly workflowType: string;
  readonly input: I;
  readonly idempotencyKey?: string;
  /**
   * Optional operator-facing display name for the workflow this start creates
   * (#211). When set, the server records it durably on the start as the
   * `aion.display_name` search attribute; omitting it starts the run unnamed
   * (it shows its bare UUID).
   *
   * A LABEL over the UUID identity, never an address: no SDK call resolves a
   * workflow by name, and no SDK helper composes name-filter → take-first →
   * act. It is trimmed before it is sent, because the server trims it before
   * recording it, and a present-but-blank name is refused rather than read as
   * "unnamed" — only ABSENCE starts a run unnamed.
   *
   * It is deliberately NOT part of the idempotency fingerprint — see
   * {@link StartOutcome.displayNameNotApplied} for what happens when a reused
   * key carries a different name.
   *
   * You name the run you are starting, but the label READS BACK per workflow:
   * the recorded attribute carries no run id, so every reader folds it over the
   * whole history, last write wins. A continue-as-new successor of this run
   * inherits the name, and describing any run of the workflow returns whatever
   * name was recorded most recently.
   */
  readonly displayName?: string;
}

export interface StartRawOptions {
  readonly namespace?: string;
  readonly workflowType: string;
  readonly input: Payload;
  readonly idempotencyKey?: string;
  /** See {@link StartOptions.displayName}. */
  readonly displayName?: string;
}

/**
 * The report that an idempotent start replay did not apply the display name
 * this call requested (#211).
 *
 * Carries both sides so the caller can act on the difference: use a different
 * idempotency key, accept the standing name, or rename the run through the
 * console/HTTP rename surface (renaming is not an SDK operation in this
 * release).
 */
export interface DisplayNameNotApplied {
  /**
   * The display name this start asked for. Never blank and never absent: the
   * report is raised only for a call that actually requested a name, so there
   * is always a name that did not get applied.
   */
  readonly requested: string;
  /**
   * The display name the CACHED start requested for that run (`undefined` when
   * that start asked for no name).
   *
   * This is what the first start asked for, not a live read: the cache is never
   * refreshed, so if the run has since been renamed, this is the name it was
   * STARTED with rather than the one it now wears. Read it as "the name this
   * key already stood for", and read the run itself if you need its current
   * name.
   */
  readonly standing: string | undefined;
}

/**
 * What {@link Client.start} returns: the started (or idempotently replayed)
 * run, plus the note that a requested display name was NOT applied.
 *
 * The note exists because `displayName` is deliberately outside the idempotency
 * fingerprint: the fingerprint answers "is this the same act?", and a label
 * carries no identity, so two starts differing only in name are the same act
 * and dedupe to one run. That leaves a second, different name with nowhere to
 * go — and dropping it silently would hand the caller a run wearing a name they
 * did not ask for while saying nothing. So the drop is REPORTED here instead.
 * The replayed run keeps its EXISTING name; nothing is renamed behind the
 * caller's back.
 */
export interface StartOutcome {
  /**
   * Handle to the started run, or to the existing run an idempotent replay
   * matched.
   */
  readonly handle: WorkflowHandle;
  /**
   * Present ONLY when this call REQUESTED a display name, an idempotency-key
   * replay matched an existing run, and that run does not already wear the
   * requested name — whether it wears a different one or none at all.
   *
   * `undefined` on every other path: a first start (which applies the name it
   * asked for), a replay that requested the same name, and — the rule this
   * field's absence states — any call that requested NO name. A caller who
   * asked for nothing had nothing dropped, so there is nothing to report, even
   * when the standing run wears a name of its own.
   */
  readonly displayNameNotApplied: DisplayNameNotApplied | undefined;
}

export interface SignalOptions<I> extends WorkflowRef {
  readonly namespace?: string;
  readonly signalName: string;
  readonly payload: I;
}

export interface SignalRawOptions extends WorkflowRef {
  readonly namespace?: string;
  readonly signalName: string;
  readonly payload: Payload;
}

export interface QueryOptions<I> extends WorkflowRef {
  readonly namespace?: string;
  readonly queryName: string;
  readonly input?: I;
  readonly timeoutMs?: number;
}

export interface QueryRawOptions extends WorkflowRef {
  readonly namespace?: string;
  readonly queryName: string;
  readonly input?: Payload;
  readonly timeoutMs?: number;
}

export interface CancelOptions extends WorkflowRef {
  readonly namespace?: string;
  readonly reason?: string;
}

export interface ListOptions {
  readonly namespace?: string;
  readonly filter?: WireEnvelope;
}

export interface DescribeOptions extends WorkflowRef {
  readonly namespace?: string;
  readonly includeHistory?: boolean;
}

export interface WorkflowDescription {
  readonly summary?: WireEnvelope;
  readonly history: readonly WireEnvelope[];
}

export interface ListWorkflowsResult {
  readonly summaries: readonly WireEnvelope[];
}

export interface WorkflowTransport {
  start(
    request: ProtoStartWorkflowRequest,
  ): Promise<ProtoStartWorkflowResponse>;
  signal(request: ProtoSignalRequest): Promise<void>;
  query(request: ProtoQueryRequest): Promise<ProtoQueryResponse>;
  cancel(request: ProtoCancelRequest): Promise<void>;
  list(request: ProtoListWorkflowsRequest): Promise<ProtoListWorkflowsResponse>;
  describe(
    request: ProtoDescribeWorkflowRequest,
  ): Promise<ProtoDescribeWorkflowResponse>;
}

export interface ProtoWorkflowId {
  readonly uuid: string;
}

export interface ProtoRunId {
  readonly uuid: string;
}

export interface ProtoStartWorkflowRequest {
  readonly namespace: string;
  readonly workflow_type: string;
  readonly input: Payload;
  /**
   * #211. Absent is how the wire says "unnamed": the server refuses a
   * present-but-blank name with `invalid_input` rather than reinterpreting one
   * as unnamed, so this key is omitted entirely rather than sent empty.
   */
  readonly display_name?: string;
}

export interface ProtoStartWorkflowResponse {
  readonly workflow_id?: ProtoWorkflowId;
  readonly run_id?: ProtoRunId;
}

export interface ProtoSignalRequest {
  readonly namespace: string;
  readonly workflow_id: ProtoWorkflowId;
  readonly run_id?: ProtoRunId;
  readonly signal_name: string;
  readonly payload: Payload;
}

export interface ProtoQueryRequest {
  readonly namespace: string;
  readonly workflow_id: ProtoWorkflowId;
  readonly run_id?: ProtoRunId;
  readonly query_name: string;
}

export interface ProtoQueryResponse {
  readonly result?: Payload;
  readonly error?: unknown;
}

export interface ProtoCancelRequest {
  readonly namespace: string;
  readonly workflow_id: ProtoWorkflowId;
  readonly run_id?: ProtoRunId;
  readonly reason?: string;
}

export interface ProtoListWorkflowsRequest {
  readonly namespace: string;
  readonly filter?: WireEnvelope;
}

export interface ProtoListWorkflowsResponse {
  readonly summaries?: readonly WireEnvelope[];
}

export interface ProtoDescribeWorkflowRequest {
  readonly namespace: string;
  readonly workflow_id: ProtoWorkflowId;
  readonly run_id?: ProtoRunId;
  readonly include_history: boolean;
}

export interface ProtoDescribeWorkflowResponse {
  readonly summary?: WireEnvelope;
  readonly history?: readonly WireEnvelope[];
}

export async function connect(options: ClientOptions): Promise<Client> {
  const client = new Client(options);
  return client;
}

export class Client {
  private readonly endpoint: string;
  private readonly namespace: string;
  private readonly transport: WorkflowTransport;
  readonly streamTransport: SubscribeTransport;
  /**
   * SDK-boundary start idempotency (the contract's hard case): the same key
   * retried with an identical request returns the original handle;
   * conflicting reuse throws {@link AlreadyExistsError}. Keyed by
   * idempotency key, fingerprinted over namespace, workflow type, and the
   * encoded input payload.
   *
   * The requested display name is carried BESIDE the fingerprint, never inside
   * it (#211). Inside, it would make two starts differing only in a label two
   * different acts and break the dedupe ruling; absent entirely, a replay could
   * not tell the caller which name the standing run actually wears. Beside is
   * what lets the replay dedupe AND report — see {@link StartOutcome}.
   */
  private readonly idempotentStarts = new Map<string, CachedStart>();

  constructor(options: ClientOptions) {
    this.endpoint = normalizeEndpoint(options.endpoint, options.tls);
    this.namespace = options.namespace ?? "default";
    this.transport =
      options.transport ??
      new HttpWorkflowTransport(
        this.endpoint,
        options.auth,
        options.fetch ?? fetch,
      );
    // An injected transport wins; otherwise subscriptions ride the built-in
    // WebSocket transport against the same listener as the HTTP endpoint.
    this.streamTransport =
      options.streamTransport ??
      new WebSocketSubscribeTransport({
        endpoint: this.endpoint,
        auth: options.auth,
      });
  }

  /**
   * Starts a workflow and returns the assigned run.
   *
   * Returns a {@link StartOutcome} rather than a bare handle so an idempotent
   * replay can report a display name it did NOT apply (#211) instead of
   * dropping it silently — see {@link StartOutcome.displayNameNotApplied}.
   */
  async start<I>(options: StartOptions<I>): Promise<StartOutcome> {
    return this.startRaw({
      ...options,
      input: toPayload(options.input),
    });
  }

  async startRaw(options: StartRawOptions): Promise<StartOutcome> {
    // #211: the server REFUSES a present-but-blank display name with
    // `invalid_input` — it does not reinterpret one as "unnamed", the way it
    // reinterprets a blank `task_queue` as "no selection". Refusing here too
    // means a caller who passed `"  "` gets the same answer from the SDK as
    // from the raw HTTP route, and gets it before a round trip. Omitting the
    // option is how a caller says "no name"; absence is what the server accepts
    // as unnamed.
    if (
      options.displayName !== undefined &&
      options.displayName.trim().length === 0
    ) {
      throw new InvalidArgumentError(
        "displayName must not be blank; omit it to start the run unnamed",
      );
    }
    // The server trims the name before recording it, so the run wears the
    // TRIMMED string. Trim here too and the SDK's cached "standing name" is the
    // name the run actually has — otherwise a replay comparing `"X"` against a
    // cached `"  X  "` would report a difference the server had already erased.
    // Blank-after-trim was refused above, so this can never turn a name into no
    // name. One trimmed value is then what is SENT, what is STORED in the cache
    // entry, and what is COMPARED on a replay — one value, three uses, so they
    // cannot disagree.
    const displayName =
      options.displayName === undefined ? undefined : options.displayName.trim();
    const namespace = this.resolveNamespace(options.namespace);
    // `displayName` is deliberately absent from the fingerprint: a label
    // carries no identity, so two starts differing only in name are the SAME
    // act and must dedupe to one run rather than conflict.
    const fingerprint =
      options.idempotencyKey === undefined
        ? undefined
        : JSON.stringify([
            namespace,
            options.workflowType,
            options.input.content_type,
            options.input.bytes,
          ]);
    if (options.idempotencyKey !== undefined && fingerprint !== undefined) {
      const cached = this.idempotentStarts.get(options.idempotencyKey);
      if (cached !== undefined) {
        if (cached.fingerprint === fingerprint) {
          // The run already exists and keeps the name it already wears. If this
          // call asked for a different one, say so rather than quietly handing
          // back a differently-named run (#211).
          return {
            handle: cached.handle,
            displayNameNotApplied: displayNameNotApplied(cached, displayName),
          };
        }
        throw idempotencyConflict();
      }
    }
    const response = await this.transport.start({
      namespace,
      workflow_type: options.workflowType,
      input: options.input,
      ...(displayName === undefined ? {} : { display_name: displayName }),
    });
    const workflowId = response.workflow_id?.uuid;
    const runId = response.run_id?.uuid;
    if (workflowId === undefined || runId === undefined) {
      throw mapWireError({
        code: "server",
        message: "Start response omitted workflow_id or run_id",
      });
    }
    const handle = new WorkflowHandle(this, workflowId, runId, namespace);
    if (options.idempotencyKey !== undefined && fingerprint !== undefined) {
      const recorded = this.idempotentStarts.get(options.idempotencyKey);
      if (recorded !== undefined && recorded.fingerprint !== fingerprint) {
        throw idempotencyConflict();
      }
      if (recorded === undefined) {
        this.idempotentStarts.set(options.idempotencyKey, {
          fingerprint,
          handle,
          displayName,
        });
      }
    }
    // This call reached the server, so whatever name it asked for was applied
    // to the run it started: there is nothing unapplied to report. That holds
    // even when a concurrent start won the cache race above — the handle
    // returned here is THIS start's own run, wearing THIS call's own name.
    return { handle, displayNameNotApplied: undefined };
  }

  async signal<I>(options: SignalOptions<I>): Promise<void> {
    await this.signalRaw({ ...options, payload: toPayload(options.payload) });
  }

  async signalRaw(options: SignalRawOptions): Promise<void> {
    await this.transport.signal({
      namespace: this.resolveNamespace(options.namespace),
      workflow_id: { uuid: options.workflowId },
      run_id: runId(options.runId),
      signal_name: options.signalName,
      payload: options.payload,
    });
  }

  async query<I, R>(options: QueryOptions<I>): Promise<R> {
    const raw = await this.queryRaw({
      ...options,
      input: options.input === undefined ? undefined : toPayload(options.input),
    });
    return fromPayload<R>(raw);
  }

  async queryRaw(options: QueryRawOptions): Promise<Payload> {
    const response = await this.queryWithDeadline(
      {
        namespace: this.resolveNamespace(options.namespace),
        workflow_id: { uuid: options.workflowId },
        run_id: runId(options.runId),
        query_name: options.queryName,
      },
      options.timeoutMs,
    );
    if (response.error !== undefined) {
      throw mapWireError(response.error);
    }
    if (response.result === undefined) {
      throw mapWireError({
        code: "server",
        message: "Query response omitted result and error",
      });
    }
    return response.result;
  }

  /**
   * Bounds one query round-trip by the caller's deadline. Query is a
   * synchronous deadline-bounded round-trip per the client contract:
   * deadline expiry surfaces {@link QueryTimeoutError} (the in-flight
   * transport call's eventual result is discarded).
   */
  private async queryWithDeadline(
    request: ProtoQueryRequest,
    timeoutMs?: number,
  ): Promise<ProtoQueryResponse> {
    if (timeoutMs === undefined) {
      return this.transport.query(request);
    }
    if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
      throw new InvalidArgumentError(
        `timeoutMs must be a positive number of milliseconds; got ${String(timeoutMs)}`,
      );
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const deadline = new Promise<never>((_, reject) => {
      timer = setTimeout(() => {
        reject(
          new QueryTimeoutError(
            `Query deadline of ${String(timeoutMs)}ms elapsed before a result was available`,
          ),
        );
      }, timeoutMs);
    });
    const call = this.transport.query(request);
    // Once the deadline wins the race, the losing call's eventual rejection
    // must never surface as an unhandled rejection.
    void call.catch(() => undefined);
    try {
      return await Promise.race([call, deadline]);
    } finally {
      if (timer !== undefined) {
        clearTimeout(timer);
      }
    }
  }

  async cancel(options: CancelOptions): Promise<void> {
    await this.transport.cancel({
      namespace: this.resolveNamespace(options.namespace),
      workflow_id: { uuid: options.workflowId },
      run_id: runId(options.runId),
      reason: options.reason,
    });
  }

  async list(options: ListOptions = {}): Promise<ListWorkflowsResult> {
    const response = await this.transport.list({
      namespace: this.resolveNamespace(options.namespace),
      filter: options.filter,
    });
    return { summaries: response.summaries ?? [] };
  }

  async describe(options: DescribeOptions): Promise<WorkflowDescription> {
    const response = await this.transport.describe({
      namespace: this.resolveNamespace(options.namespace),
      workflow_id: { uuid: options.workflowId },
      run_id: runId(options.runId),
      include_history: options.includeHistory ?? false,
    });
    return {
      summary: response.summary,
      history: response.history ?? [],
    };
  }

  handle(
    workflowId: string,
    runId?: string,
    namespace?: string,
  ): WorkflowHandle {
    return new WorkflowHandle(
      this,
      workflowId,
      runId,
      this.resolveNamespace(namespace),
    );
  }

  resolveNamespace(namespace?: string): string {
    return namespace ?? this.namespace;
  }
}

class HttpWorkflowTransport implements WorkflowTransport {
  private readonly endpoint: string;
  private readonly auth?: AuthOptions;
  private readonly fetchImpl: typeof fetch;

  constructor(
    endpoint: string,
    auth: AuthOptions | undefined,
    fetchImpl: typeof fetch,
  ) {
    this.endpoint = endpoint;
    this.auth = auth;
    this.fetchImpl = fetchImpl;
  }

  async start(
    request: ProtoStartWorkflowRequest,
  ): Promise<ProtoStartWorkflowResponse> {
    return this.post<ProtoStartWorkflowResponse>("/workflows/start", request);
  }

  async signal(request: ProtoSignalRequest): Promise<void> {
    await this.post<unknown>("/workflows/signal", request);
  }

  async query(request: ProtoQueryRequest): Promise<ProtoQueryResponse> {
    // The HTTP route carries the QueryResponse outcome oneof as
    // `{"outcome":{"Result": <Payload>}}` or `{"outcome":{"Error":
    // <WireError>}}`; normalize it onto the transport-level result/error
    // shape the client decodes.
    const response = await this.post<{
      readonly outcome?: { readonly Result?: Payload; readonly Error?: unknown };
    }>("/workflows/query", request);
    if (response.outcome?.Error !== undefined) {
      return { error: response.outcome.Error };
    }
    return { result: response.outcome?.Result };
  }

  async cancel(request: ProtoCancelRequest): Promise<void> {
    await this.post<unknown>("/workflows/cancel", request);
  }

  async list(
    request: ProtoListWorkflowsRequest,
  ): Promise<ProtoListWorkflowsResponse> {
    return this.post<ProtoListWorkflowsResponse>("/workflows/list", request);
  }

  async describe(
    request: ProtoDescribeWorkflowRequest,
  ): Promise<ProtoDescribeWorkflowResponse> {
    return this.post<ProtoDescribeWorkflowResponse>(
      "/workflows/describe",
      request,
    );
  }

  private async post<T>(path: string, body: unknown): Promise<T> {
    let requestBody: string;
    try {
      requestBody = JSON.stringify(body);
    } catch (error) {
      throw new InvalidArgumentError("Failed to serialize request body", {
        cause: error,
      });
    }

    let response: Response;
    try {
      response = await this.fetchImpl(new URL(path, `${this.endpoint}/`), {
        method: "POST",
        headers: this.headers(),
        body: requestBody,
      });
    } catch (error) {
      throw mapTransportError(error);
    }

    if (!response.ok) {
      throw await mapHttpResponseError(response);
    }

    try {
      const text = await response.text();
      if (text.length === 0) {
        return undefined as T;
      }
      return JSON.parse(text) as T;
    } catch (error) {
      throw mapWireError({
        code: "server",
        message: "Failed to decode server response",
        detail: error,
      });
    }
  }

  private headers(): Record<string, string> {
    return {
      "content-type": "application/json",
      ...authHeaders(this.auth),
    };
  }
}

/**
 * One idempotency-cache entry: the fingerprint the start was keyed by, the
 * handle to replay, and the display name that start requested (#211).
 */
interface CachedStart {
  readonly fingerprint: string;
  readonly handle: WorkflowHandle;
  readonly displayName: string | undefined;
}

/**
 * The report that `requested` was not applied to `cached`'s run, or `undefined`
 * when there is nothing to report.
 *
 * The rule, exactly: a report is raised when this replay ASKED for a name
 * (`requested !== undefined`) that the standing run does not already wear —
 * whether that run wears a different name or no name at all. Both of those
 * callers asked for something the replay could not give them, so both are told.
 *
 * A replay that asked for NO name gets no report, whatever the standing run
 * wears. Nothing was requested, so nothing was dropped, and the report's absence
 * is precisely the statement that no requested name went missing — see
 * {@link StartOutcome.displayNameNotApplied}. Raising it there would report a
 * drop that never happened and push callers who branch on the field into acting
 * on a run they never tried to name.
 */
function displayNameNotApplied(
  cached: CachedStart,
  requested: string | undefined,
): DisplayNameNotApplied | undefined {
  if (requested === undefined || cached.displayName === requested) {
    return undefined;
  }
  return { requested, standing: cached.displayName };
}

/**
 * The SDK-boundary idempotency conflict: the same key was reused with a
 * different start request.
 */
function idempotencyConflict(): AlreadyExistsError {
  return new AlreadyExistsError(
    "idempotency key was already used by a different start request " +
      "(namespace, workflow type, or input differ)",
  );
}

function normalizeEndpoint(endpoint: string, tls?: TlsOptions): string {
  let url: URL;
  try {
    url = new URL(endpoint);
  } catch (error) {
    throw new InvalidArgumentError("Client endpoint must be an absolute URL", {
      cause: error,
    });
  }
  if (tls?.enabled === true && url.protocol === "http:") {
    url.protocol = "https:";
  }
  return url.toString().replace(/\/$/, "");
}

function runId(value: string | undefined): ProtoRunId | undefined {
  return value === undefined ? undefined : { uuid: value };
}
