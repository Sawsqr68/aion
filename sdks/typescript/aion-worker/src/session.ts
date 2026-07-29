import { hostname } from "node:os";
import * as grpc from "@grpc/grpc-js";
import type {
	ActivityError,
	ActivityTask as WireActivityTask,
	RegisterAck as WireRegisterAck,
	ResultAck as WireResultAck,
	ServerToWorker,
	WorkerToServer,
} from "./proto/worker.js";
import { ActivityErrorKind } from "./proto/worker.js";
import type { ActivityId, Payload as WirePayload } from "./proto/common.js";
import {
	type GrpcClientFactory,
	grpcClientFactory,
	type WorkerProtocolTransport,
	type WorkerStream,
	type WorkerStreamMetadata,
} from "./transport.js";

export type WorkerIdentity = string;

/**
 * A JSON Schema document, as an ordinary JSON value.
 *
 * The wire carries these as strings (`ActivityDescriptor.input_schema_json`),
 * and the server parses them with `serde_json` and returns `InvalidArgument`
 * on anything unparseable. Modelling them as `unknown`-shaped JSON here — not
 * as a hand-rolled schema type — keeps the SDK from pretending to understand
 * a dialect it does not validate.
 */
export type JsonSchema =
	| boolean
	| { readonly [key: string]: unknown };

/**
 * The permissive schema: accepts every value.
 *
 * Used as the ADVERTISED INPUT schema when an activity declares no input
 * shape. This is sound, and it is the truth rather than a convenient fiction:
 * {@link defineActivity} decodes with `JSON.parse` and an unchecked cast, so
 * the handler genuinely accepts any JSON the server sends.
 *
 * It is sound against the server's matcher too, because the two schema checks
 * have OPPOSITE variance (`aion_package::contract_diffs`):
 *
 *   input:  schema_is_subset(contract_input,  advertised_input)
 *   output: schema_is_subset(advertised_output, contract_output)
 *
 * The worker's schema is the SUPERSET side for input, so "I accept anything"
 * satisfies every contract. It is the SUBSET side for output, where the same
 * value would be refused — see {@link ActivityContract.output}.
 */
export const PERMISSIVE_SCHEMA: JsonSchema = true;

/**
 * The typed action surface an activity commits to, advertised in
 * `RegisterWorker.activities` and matched against every deployed `.v4`
 * contract for the queue before the worker becomes dispatch-visible.
 *
 * TypeScript types are erased at runtime, so — unlike the Rust worker, which
 * derives both schemas from the handler's concrete serde types via schemars —
 * this SDK has NO way to discover a handler's shape. The schemas here are the
 * author's assertion, taken on trust. That trust boundary is the price of an
 * erased-type language and is documented rather than hidden.
 */
export interface ActivityContract {
	/**
	 * JSON Schema the handler accepts. Optional: when omitted the worker
	 * advertises {@link PERMISSIVE_SCHEMA}, which is both honest (the handler
	 * really does accept any JSON) and accepted by the server's contravariant
	 * input check.
	 */
	readonly input?: JsonSchema;
	/**
	 * JSON Schema the handler produces. REQUIRED, and deliberately so.
	 *
	 * The server's output check is covariant — the worker's schema must be a
	 * SUBSET of the contract's — so a permissive output schema is refused by
	 * any contract that declares a real record type, and `false` ("produces
	 * nothing") sneaks past the matcher only by lying in the other direction.
	 * There is therefore no honest value this SDK could supply on the author's
	 * behalf. Omitting it is refused locally, at registration, naming the
	 * activity — an error the author can act on, instead of an opaque
	 * FAILED_PRECONDITION schema diff from a remote server.
	 */
	readonly output: JsonSchema;
}

/**
 * One entry of the worker's advertised typed action surface, ready for the
 * wire. Schemas are JSON values here and are serialized at the wire boundary.
 */
export interface ActivityDescriptor {
	readonly name: string;
	readonly input: JsonSchema;
	readonly output: JsonSchema;
}

/**
 * Everything a worker advertises at registration beyond its connection config.
 *
 * `activities` is the typed action surface. An EMPTY array is a meaningful
 * wire value, not a missing one: the proto defines it as "this worker predates
 * contract commitment", and the server refuses such a worker on any queue with
 * a deployed contract. A PARTIAL array has no defined meaning, which is why
 * {@link ActivityRegistry} refuses to build one.
 */
export interface WorkerAdvertisement {
	readonly activityTypes: readonly string[];
	readonly activities: readonly ActivityDescriptor[];
}

/**
 * Operator-supplied reconnect/backoff policy governing both session
 * establishment and the run loop's cumulative mid-run session-drop budget
 * of `maxAttempts`.
 *
 * Budget reset: the cumulative drop budget resets to zero once an
 * established session proves healthy — it served at least one task, or it
 * stayed connected longer than `maxDelayMs` (measured monotonically from
 * successful registration to the moment the stream ended or dropped;
 * post-drop draining of in-flight handlers never extends it). The cap is
 * the policy's own definition of the longest pause, so a session outliving
 * it is demonstrably past the flapping regime, and a served task proves
 * end-to-end health. A genuinely flapping server — no session ever serves a
 * task or outlives `maxDelayMs` — exhausts the budget after exactly
 * `maxAttempts` drops.
 *
 * Drains and clean closes: a server-announced drain (the wire
 * `DrainRequest` frame) is an unbudgeted drop — the worker finishes
 * in-flight work and redials after `initialDelayMs`; the drain
 * classification latches for the session, so even an abrupt end after the
 * frame stays drain-class. An *unannounced* clean stream close remains a
 * budgeted retryable drop: the worker redials through the same budgeted,
 * backed-off cycle, and only a persistent unannounced clean-close loop
 * exhausts the budget (surfacing `ReconnectExhaustedError` with a
 * `ServerClosedStreamError` cause).
 *
 * Shutdown during establishment or a backoff: shutdown wins promptly during
 * BOTH backoff phases — the session-establishment retries and the mid-run
 * drop backoffs — AND during an in-flight establishment attempt. Every SDK
 * races each backoff sleep and the whole dial/handshake/register chain
 * against the shutdown signal and never dials again once it fires (the Rust
 * worker selects shutdown around the entire establishment; this SDK and the
 * Python worker race the attempt and close an attempt abandoned to shutdown
 * when it eventually settles). The run outcome is aligned across the Rust,
 * Python, and TypeScript workers: a pending drain-class or clean-close drop
 * ends the run cleanly, while a pending error-class drop surfaces its error
 * — a supervisor sees "this worker was mid-fault" distinctly from "this
 * worker drained cleanly".
 */
export interface ReconnectConfig {
	readonly initialDelayMs: number;
	/**
	 * Backoff delay cap. Doubles as the session-health threshold for the
	 * drop-budget reset described on this type.
	 */
	readonly maxDelayMs: number;
	readonly maxAttempts: number;
}

/**
 * Worker connection and serving configuration.
 *
 * `namespaces`, `taskQueue`, and `node` are DISJOINT routing dimensions and
 * must not be conflated (they were, until recently: this SDK used to send the
 * task queue in a since-deleted `namespace` field, which is why nothing it
 * registered could be routed).
 *
 *   namespaces — the SET of correctness/isolation boundaries. A workflow's
 *                activities only ever reach workers whose set includes the
 *                workflow's namespace, so crossing one is a bug, never a
 *                tuning choice. EVERY entry must be authorized for the caller
 *                or the whole registration is denied.
 *   taskQueue  — the pool/flavour selector WITHIN each namespace (norn /
 *                claude / cpu / gpu). A miss here is a liveness problem, never
 *                a correctness one.
 *   node       — OPTIONAL locality affinity. A dispatch pinned to a node
 *                reaches only workers advertising it.
 */
export interface WorkerConfig {
	readonly endpoint: string;
	/**
	 * Correctness/isolation boundaries this worker registers into, sent both
	 * as the `x-aion-namespaces` stream metadata and as
	 * `RegisterWorker.namespaces` so authorization scope and registration
	 * scope can never disagree. Must be non-empty.
	 *
	 * Required, deliberately diverging from the Rust worker's builder, which
	 * defaults an unset value to `["default"]`. That default is safe behind a
	 * builder whose other fields are set one call at a time; a TypeScript
	 * config is a single object literal where every field is already spelled
	 * out, so defaulting would only hide a silent choice of correctness
	 * boundary. A compile error is the cheapest possible place to catch it.
	 */
	readonly namespaces: readonly string[];
	/** Auth subject sent as `x-aion-subject` stream metadata. Must be non-empty. */
	readonly subject: string;
	/** Pool/flavour selector within each namespace. Must be non-empty. */
	readonly taskQueue: string;
	/**
	 * Locality affinity advertised at registration. Defaults to the machine
	 * hostname, falling back to `"localhost"` when the OS reports none —
	 * mirroring the Rust worker's `default_node()`. Pass an empty string to
	 * advertise NO locality, which makes the worker reachable only for
	 * unpinned dispatches.
	 */
	readonly node?: string;
	readonly identity: WorkerIdentity;
	readonly maxConcurrency: number;
	readonly credentials?: grpc.ChannelCredentials;
	readonly channelOptions?: grpc.ChannelOptions;
	/**
	 * Reconnect/backoff policy. Required: the worker's run loop is
	 * reconnect-aware and refuses to operate without an operator-supplied
	 * budget (there are no SDK defaults), so a missing policy is a compile
	 * error rather than a runtime rejection.
	 */
	readonly reconnect: ReconnectConfig;
}

export interface Payload {
	readonly contentType: string;
	readonly bytes: Uint8Array;
}

export type WorkflowId = string;
export type ActivityIdKey = string;

/**
 * The completion identity a result MUST carry, echoed verbatim from the
 * `ActivityTask` that produced it.
 *
 * Bundled into one type rather than passed as loose arguments so that
 * reporting an outcome WITHOUT the fencing identity is not expressible. The
 * server rejects a result missing either half unconditionally — see
 * `ActivityCompletion::try_from` ("activity result run id is missing") and
 * `CompletionToken::from_wire` (`MissingCompletionToken`) — so a session API
 * that let them be forgotten would produce a worker whose every result is
 * silently discarded.
 */
export interface ActivityCompletionIdentity {
	readonly workflowId: WorkflowId;
	readonly activityId: ActivityIdKey;
	/** Concrete run that owns the task. Echoed, never synthesized. */
	readonly runId: string;
	/**
	 * Opaque proof of this execution generation. Echoed VERBATIM: it must
	 * never be parsed, reconstructed, or inferred — its contents carry no
	 * meaning to a worker.
	 */
	readonly completionToken: string;
}

export interface ActivityTask {
	readonly workflowId: WorkflowId;
	readonly activityId: ActivityIdKey;
	readonly activityType: string;
	readonly input: Payload;
	/** One-based delivery attempt stamped by the engine. Never zero. */
	readonly attempt: number;
	/** Concrete run that owns this task. */
	readonly runId: string;
	/** Opaque execution-generation proof, echoed verbatim on the outcome. */
	readonly completionToken: string;
	/**
	 * Stable external-effect key for this run and action site. Identical
	 * across retries and across execution generations, so a handler can use it
	 * to deduplicate a side effect it performs itself.
	 */
	readonly idempotencyKey: string;
	/**
	 * Display labels the workflow attached to the activity (for example
	 * `brief=IP-001`). Display metadata only: the engine never interprets
	 * them and they never affect routing or replay.
	 */
	readonly labels: Readonly<Record<string, string>>;
}

/** The completion identity carried by a task, for echoing on its outcome. */
export function completionIdentity(task: ActivityTask): ActivityCompletionIdentity {
	return {
		workflowId: task.workflowId,
		activityId: task.activityId,
		runId: task.runId,
		completionToken: task.completionToken,
	};
}

export interface ActivityFailure {
	readonly retryable: boolean;
	readonly message: string;
	readonly details?: Payload;
}

export type WorkerSessionEvent =
	| { readonly kind: "task"; readonly task: ActivityTask }
	/**
	 * The server consumed the identified `ActivityResult` frame; the worker
	 * may stop re-reporting it. Clears the matching unacked-tracker entry.
	 */
	| {
			readonly kind: "resultAck";
			readonly workflowId: WorkflowId;
			readonly activityId: ActivityIdKey;
	  }
	/**
	 * Server-initiated drain: the server is going away. The worker finishes
	 * in-flight work, stops expecting new tasks, and reconnects after the
	 * schedule's initial backoff without consuming drop budget.
	 */
	| { readonly kind: "drained" }
	| { readonly kind: "closed" };

/** Server-assigned registration facts carried by the `RegisterAck` frame. */
export interface RegisteredSessionInfo {
	readonly workerId: number;
	/**
	 * The server's comma-joined echo of the namespaces it authorized this
	 * registration against.
	 *
	 * INFORMATIONAL ONLY — named `namespaceEcho` rather than `namespace` so it
	 * cannot be mistaken for a routing input. Authorization may DERIVE this
	 * scope rather than copy it, so it is not necessarily equal to the
	 * configured set, and no worker behaviour is derived from it: it exists so
	 * a worker's logs can be correlated with the server's.
	 */
	readonly namespaceEcho: string;
	readonly heartbeatWindowMs: number;
}

export interface WorkerSession {
	handshake(config: WorkerConfig): Promise<void>;
	register(advertisement: WorkerAdvertisement): Promise<void>;
	receiveTasks(): AsyncIterable<WorkerSessionEvent>;
	reportResult(
		completion: ActivityCompletionIdentity,
		result: Payload,
	): Promise<void>;
	reportFailure(
		completion: ActivityCompletionIdentity,
		failure: ActivityFailure,
	): Promise<void>;
	sendHeartbeat(
		workflowId: WorkflowId,
		activityId: ActivityIdKey,
		progress?: Payload,
	): Promise<void>;
	/**
	 * Server-assigned liveness window from the `RegisterAck`, in
	 * milliseconds, once registered. The serve loop derives its AUTOMATIC
	 * liveness-heartbeat cadence from this window: the server's heartbeat
	 * sweeper expires any worker whose in-flight task goes longer than the
	 * window without a heartbeat, so the runtime — not each handler — keeps
	 * every in-flight activity beating. Optional so fakes stay minimal;
	 * `undefined` (or an absent method) disables the automatic pump.
	 */
	heartbeatWindowMs?(): number | undefined;
	close(): Promise<void>;
}

export type WorkerSessionFactory = (
	config: WorkerConfig,
) => Promise<WorkerSession>;

export class GrpcWorkerSession implements WorkerSession {
	private readonly stream: WorkerStream;
	/**
	 * The ONE iterator over the response stream: `register` consumes frame 1
	 * (the `RegisterAck`) from it and `receiveTasks` continues it, so no
	 * frame can be lost between the two phases.
	 */
	private readonly frames: AsyncIterator<ServerToWorker>;
	private config?: WorkerConfig;
	private registeredInfoValue?: RegisteredSessionInfo;
	private streamEnded = false;

	/**
	 * Opens the bidirectional stream immediately, attaching the authorization
	 * metadata — the server resolves the caller from that metadata BEFORE
	 * reading the first frame, so it cannot be supplied later at `handshake`.
	 */
	public constructor(
		transport: WorkerProtocolTransport,
		metadata: WorkerStreamMetadata,
	) {
		this.stream = transport.streamWorker(metadata);
		this.frames = this.stream[Symbol.asyncIterator]();
	}

	/** Server-assigned registration facts, available once registered. */
	public get registeredInfo(): RegisteredSessionInfo | undefined {
		return this.registeredInfoValue;
	}

	/**
	 * Server-assigned liveness window from the `RegisterAck`, in
	 * milliseconds. `undefined` before registration (and for a degenerate
	 * zero window) disables the serve loop's automatic liveness pump.
	 */
	public heartbeatWindowMs(): number | undefined {
		const windowMs = this.registeredInfoValue?.heartbeatWindowMs;
		if (windowMs === undefined || windowMs <= 0) {
			return undefined;
		}
		return windowMs;
	}

	public async handshake(config: WorkerConfig): Promise<void> {
		validateWorkerConfig(config);
		this.config = config;
	}

	/**
	 * Sends `RegisterWorker` and awaits the server's `RegisterAck` — the
	 * guaranteed first frame on the response stream. Registration succeeds
	 * only when the ack arrives; the wait is bounded by the reconnect
	 * policy's `maxDelayMs` (the operator's own definition of the longest
	 * tolerable pause). A timeout, a non-ack first frame, or a stream that
	 * ends before the ack is a retryable registration failure; denials fail
	 * the call with a gRPC error status exactly as before.
	 *
	 * All six `RegisterWorker` fields are populated, mirroring the Rust
	 * worker's `register_with_contract`.
	 */
	public async register(advertisement: WorkerAdvertisement): Promise<void> {
		const registeredTypes = validateActivityTypes(advertisement.activityTypes);
		const config = this.config;
		if (config === undefined) {
			throw new Error("worker session must handshake before register");
		}
		validateDescriptors(advertisement.activities, registeredTypes);
		await this.write({
			register: {
				namespaces: [...config.namespaces],
				activityTypes: [...registeredTypes],
				taskQueue: config.taskQueue,
				node: resolveNode(config),
				activities: advertisement.activities.map((activity) => ({
					name: activity.name,
					inputSchemaJson: JSON.stringify(activity.input),
					outputSchemaJson: JSON.stringify(activity.output),
				})),
				identity: config.identity,
			},
		});
		const first = await withDeadline(
			this.frames.next(),
			config.reconnect.maxDelayMs,
			`server did not acknowledge registration within ${String(config.reconnect.maxDelayMs)}ms`,
		);
		if (first.done === true) {
			throw new Error(
				"server ended the stream before acknowledging registration",
			);
		}
		const ack = first.value.registerAck;
		if (ack === undefined) {
			throw new Error(
				"protocol violation: server sent a non-RegisterAck frame before acknowledging registration",
			);
		}
		this.registeredInfoValue = decodeRegisterAck(ack);
	}

	public async *receiveTasks(): AsyncIterable<WorkerSessionEvent> {
		for (;;) {
			const next = await this.frames.next();
			if (next.done === true) {
				break;
			}
			const message = next.value;
			if (message.task !== undefined) {
				yield { kind: "task", task: decodeTask(message.task) };
			} else if (message.resultAck !== undefined) {
				yield decodeResultAck(message.resultAck);
			} else if (message.drain !== undefined) {
				yield { kind: "drained" };
			} else if (message.registerAck !== undefined) {
				// The ack is consumed inside register(); a second one
				// mid-stream is a server ordering bug that must surface.
				throw new Error(
					"protocol violation: RegisterAck received after registration completed",
				);
			}
		}
		yield { kind: "closed" };
	}

	public async reportResult(
		completion: ActivityCompletionIdentity,
		result: Payload,
	): Promise<void> {
		await this.write({
			result: {
				...encodeCompletion(completion),
				result: encodePayload(result),
			},
		});
	}

	public async reportFailure(
		completion: ActivityCompletionIdentity,
		failure: ActivityFailure,
	): Promise<void> {
		await this.write({
			result: {
				...encodeCompletion(completion),
				error: encodeFailure(failure),
			},
		});
	}

	public async sendHeartbeat(
		workflowId: WorkflowId,
		activityId: ActivityIdKey,
		progress?: Payload,
	): Promise<void> {
		await this.write({
			heartbeat: {
				workflowId: encodeWorkflowId(workflowId),
				activityId: encodeActivityId(activityId),
				progress: progress === undefined ? undefined : encodePayload(progress),
			},
		});
	}

	/**
	 * Ends the underlying client stream. Explicitly idempotent: the worker
	 * loop's failure path and the worker's abort handler can both close the
	 * same session in one shutdown race, so only the first call ends the
	 * stream and every later call is a no-op — the contract is pinned here
	 * (and by test) rather than relying on Node tolerating a double `end()`.
	 */
	public async close(): Promise<void> {
		if (this.streamEnded) {
			return;
		}
		this.streamEnded = true;
		this.stream.end();
	}

	/**
	 * Writes one frame with a per-send deadline of the reconnect policy's
	 * `maxDelayMs`: a flush that outlives the operator's longest tolerable
	 * pause is, by that same definition, a dead session and surfaces as a
	 * retryable transport-shaped error instead of hanging the worker forever.
	 */
	private async write(message: WorkerToServer): Promise<void> {
		const config = this.config;
		if (config === undefined) {
			throw new Error("worker session must handshake before writing");
		}
		const flushed = new Promise<void>((resolve, reject) => {
			this.stream.write(message, (error?: Error | null) => {
				if (error === undefined || error === null) {
					resolve();
				} else {
					reject(error);
				}
			});
		});
		await withDeadline(
			flushed,
			config.reconnect.maxDelayMs,
			`worker stream send did not complete within ${String(config.reconnect.maxDelayMs)}ms`,
		);
	}
}

/**
 * Races a promise against a deadline timer; elapse rejects with a
 * transport-shaped `Error` (no gRPC denial code, so it stays retryable by
 * `isRetryableSessionError`). The timer is always cleared so a resolved
 * promise never leaves a dangling timeout keeping the process alive.
 */
async function withDeadline<T>(
	pending: Promise<T>,
	deadlineMs: number,
	message: string,
): Promise<T> {
	let timer: ReturnType<typeof setTimeout> | undefined;
	const elapsed = new Promise<never>((_resolve, reject) => {
		timer = setTimeout(() => {
			reject(new Error(message));
		}, deadlineMs);
	});
	try {
		return await Promise.race([pending, elapsed]);
	} finally {
		if (timer !== undefined) {
			clearTimeout(timer);
		}
	}
}

/**
 * Fallback node id when the OS reports no hostname. A worker must never fail
 * to start over a missing hostname, so it advertises this documented value —
 * the same fallback as the Rust worker's `DEFAULT_WORKER_NODE`.
 */
const DEFAULT_WORKER_NODE = "localhost";

/**
 * The locality affinity to advertise. An explicitly configured value (INCLUDING
 * the empty string, which means "no locality") always wins; only an absent
 * value falls back to the machine hostname, mirroring `default_node()`.
 */
export function resolveNode(config: WorkerConfig): string {
	if (config.node !== undefined) {
		return config.node;
	}
	const machine = hostname();
	return machine.length > 0 ? machine : DEFAULT_WORKER_NODE;
}

export async function connectGrpcWorkerSession(
	config: WorkerConfig,
	clientFactory: GrpcClientFactory = grpcClientFactory(),
): Promise<WorkerSession> {
	validateWorkerConfig(config);
	const credentials = config.credentials ?? grpc.credentials.createInsecure();
	const transport = clientFactory.create(
		config.endpoint,
		credentials,
		config.channelOptions,
	);
	return new GrpcWorkerSession(transport, {
		namespaces: config.namespaces,
		subject: config.subject,
	});
}

function validateWorkerConfig(config: WorkerConfig): void {
	if (config.endpoint.length === 0) {
		throw new Error("worker endpoint must not be empty");
	}
	if (config.namespaces.length === 0) {
		throw new Error("worker namespaces must include at least one namespace");
	}
	for (const namespace of config.namespaces) {
		if (namespace.length === 0) {
			throw new Error("worker namespace must not be empty");
		}
		if (namespace.includes(",")) {
			// The auth metadata is comma-joined, so an embedded comma would
			// silently split one namespace into two and change the correctness
			// boundary the server authorizes.
			throw new Error("worker namespace must not contain a comma");
		}
	}
	if (config.subject.length === 0) {
		throw new Error("worker subject must not be empty");
	}
	if (config.taskQueue.length === 0) {
		throw new Error("worker taskQueue must not be empty");
	}
	if (config.identity.length === 0) {
		throw new Error("worker identity must not be empty");
	}
	if (!Number.isInteger(config.maxConcurrency) || config.maxConcurrency <= 0) {
		throw new Error("worker maxConcurrency must be a positive integer");
	}
}

function validateActivityTypes(
	activityTypes: readonly string[],
): readonly string[] {
	if (activityTypes.length === 0) {
		throw new Error(
			"worker registration must include at least one activity type",
		);
	}
	const normalizedTypes = [...activityTypes];
	for (const activityType of normalizedTypes) {
		if (activityType.length === 0) {
			throw new Error("worker activity type must not be empty");
		}
	}
	return normalizedTypes;
}

/**
 * Guards the wire meaning of `RegisterWorker.activities`.
 *
 * Empty is a DEFINED value ("predates contract commitment"); a complete set is
 * a DEFINED value ("this is my typed action surface"). A set that names only
 * some of the served activities is neither, and the server would read the gap
 * as "this worker does not serve that action" — a refusal whose message points
 * at the wrong problem. Refuse here, where the real cause is nameable.
 */
function validateDescriptors(
	activities: readonly ActivityDescriptor[],
	activityTypes: readonly string[],
): void {
	if (activities.length === 0) {
		return;
	}
	const described = new Set(activities.map((activity) => activity.name));
	const undeclared = activityTypes.filter((name) => !described.has(name));
	if (undeclared.length > 0) {
		throw new Error(
			"worker advertises a partial typed action surface: " +
				`${undeclared.join(", ")} declared no contract while ` +
				`${[...described].sort().join(", ")} did. Declare a contract for ` +
				"every activity, or for none.",
		);
	}
	const unserved = [...described].filter(
		(name) => !activityTypes.includes(name),
	);
	if (unserved.length > 0) {
		throw new Error(
			`worker advertises contracts for unserved activities: ${unserved.sort().join(", ")}`,
		);
	}
}

/**
 * Decodes a wire task, REFUSING anything malformed rather than defaulting it.
 *
 * Every field the server requires on the way back is validated on the way in,
 * so a task that could not produce a valid result is rejected at the boundary
 * instead of failing later as a silently-discarded completion. Mirrors the
 * Rust worker's `TryFrom<ProtoActivityTask>`.
 */
export function decodeTask(task: WireActivityTask): ActivityTask {
	if (task.workflowId === undefined || task.workflowId.uuid.length === 0) {
		throw new Error("activity task is missing workflow_id");
	}
	if (task.activityId === undefined) {
		throw new Error("activity task is missing activity_id");
	}
	if (task.activityType.length === 0) {
		throw new Error("activity task is missing activity_type");
	}
	if (task.input === undefined) {
		throw new Error("activity task is missing input payload");
	}
	const attempt = Number(task.attempt ?? 0);
	if (!Number.isInteger(attempt) || attempt <= 0) {
		// proto3 zero default = the producer failed to stamp the attempt.
		throw new Error(
			"activity task attempt is missing or zero (producer failed to stamp it)",
		);
	}
	if (task.runId === undefined || task.runId.uuid.length === 0) {
		// A missing run cannot produce a run-scoped completion, and the server
		// refuses any result lacking one.
		throw new Error("activity task is missing run_id");
	}
	if (task.completionToken.length === 0) {
		// Empty is not "the current generation" — it identifies a server from
		// an incompatible pre-fencing era, whose completions are all rejected.
		throw new Error("activity task is missing completion_token");
	}
	if (task.idempotencyKey.length === 0) {
		throw new Error("activity task is missing idempotency_key");
	}
	return {
		workflowId: task.workflowId.uuid,
		activityId: activityIdToKey(task.activityId),
		activityType: task.activityType,
		input: decodePayload(task.input),
		attempt,
		runId: task.runId.uuid,
		completionToken: task.completionToken,
		idempotencyKey: task.idempotencyKey,
		labels: { ...task.labels },
	};
}

function decodeResultAck(ack: WireResultAck): WorkerSessionEvent {
	if (ack.workflowId === undefined || ack.workflowId.uuid.length === 0) {
		throw new Error("result ack is missing workflow_id");
	}
	if (ack.activityId === undefined) {
		throw new Error("result ack is missing activity_id");
	}
	return {
		kind: "resultAck",
		workflowId: ack.workflowId.uuid,
		activityId: activityIdToKey(ack.activityId),
	};
}

function decodeRegisterAck(ack: WireRegisterAck): RegisteredSessionInfo {
	return {
		workerId: Number(ack.workerId ?? 0),
		namespaceEcho: ack.namespace ?? "",
		heartbeatWindowMs: Number(ack.heartbeatWindowMs ?? 0),
	};
}

export function decodePayload(payload: WirePayload): Payload {
	if (payload.contentType.length === 0) {
		throw new Error("payload is missing content_type");
	}
	return {
		contentType: payload.contentType,
		bytes: Uint8Array.from(payload.bytes),
	};
}

export function encodePayload(payload: Payload): WirePayload {
	if (payload.contentType.length === 0) {
		throw new Error("payload contentType must not be empty");
	}
	return {
		contentType: payload.contentType,
		bytes: Uint8Array.from(payload.bytes),
	};
}

export function activityIdToKey(activityId: ActivityId): ActivityIdKey {
	return String(activityId.sequencePosition);
}

/**
 * Encodes the identity every `ActivityResult` must carry. The completion token
 * is copied through untouched — never parsed, never rebuilt.
 */
function encodeCompletion(completion: ActivityCompletionIdentity): {
	readonly workflowId: { readonly uuid: string };
	readonly activityId: { readonly sequencePosition: bigint };
	readonly runId: { readonly uuid: string };
	readonly completionToken: string;
} {
	if (completion.runId.length === 0) {
		throw new Error("activity result runId must not be empty");
	}
	if (completion.completionToken.length === 0) {
		throw new Error("activity result completionToken must not be empty");
	}
	return {
		workflowId: encodeWorkflowId(completion.workflowId),
		activityId: encodeActivityId(completion.activityId),
		runId: { uuid: completion.runId },
		completionToken: completion.completionToken,
	};
}

function encodeWorkflowId(workflowId: WorkflowId): { readonly uuid: string } {
	if (workflowId.length === 0) {
		throw new Error("workflowId must not be empty");
	}
	return { uuid: workflowId };
}

function encodeActivityId(activityId: ActivityIdKey): {
	readonly sequencePosition: bigint;
} {
	if (activityId.length === 0) {
		throw new Error("activityId must not be empty");
	}
	let sequencePosition: bigint;
	try {
		sequencePosition = BigInt(activityId);
	} catch {
		throw new Error(
			`activityId must be an integer sequence position, got '${activityId}'`,
		);
	}
	return { sequencePosition };
}

function encodeFailure(failure: ActivityFailure): ActivityError {
	if (failure.message.length === 0) {
		throw new Error("activity failure message must not be empty");
	}
	return {
		kind: failure.retryable
			? ActivityErrorKind.ACTIVITY_ERROR_KIND_RETRYABLE
			: ActivityErrorKind.ACTIVITY_ERROR_KIND_TERMINAL,
		message: failure.message,
		details:
			failure.details === undefined
				? undefined
				: encodePayload(failure.details),
	};
}
