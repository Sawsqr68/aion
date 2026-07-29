import { describe, expect, it } from "vitest";
import {
	completionIdentity,
	decodePayload,
	decodeTask,
	encodePayload,
	GrpcWorkerSession,
	resolveNode,
	type ActivityTask,
	type WorkerAdvertisement,
	type WorkerConfig,
	type WorkerSession,
	type WorkerSessionEvent,
} from "./index.js";
import { workerAuthMetadata } from "./transport.js";
import type {
	ActivityTask as WireActivityTask,
	ServerToWorker,
	WorkerToServer,
} from "./proto/worker.js";

class FakeSession implements WorkerSession {
	public handshakes: WorkerConfig[] = [];
	public registrations: WorkerAdvertisement[] = [];

	public async handshake(config: WorkerConfig): Promise<void> {
		this.handshakes.push(config);
	}

	public async register(advertisement: WorkerAdvertisement): Promise<void> {
		this.registrations.push(advertisement);
	}

	public async *receiveTasks(): AsyncIterable<WorkerSessionEvent> {
		yield { kind: "closed" };
	}

	public reportResult(): Promise<void> {
		return Promise.resolve();
	}

	public reportFailure(): Promise<void> {
		return Promise.resolve();
	}

	public sendHeartbeat(): Promise<void> {
		return Promise.resolve();
	}

	public close(): Promise<void> {
		return Promise.resolve();
	}
}

/** A complete, well-formed wire task; individual tests corrupt one field. */
function wireTask(): WireActivityTask {
	return {
		workflowId: { uuid: "wf-1" },
		activityId: { sequencePosition: 7n },
		activityType: "charge",
		input: {
			contentType: "application/json",
			bytes: new Uint8Array([123, 125]),
		},
		attempt: 3,
		labels: { brief: "IP-001" },
		runId: { uuid: "run-1" },
		completionToken: "generation-9",
		idempotencyKey: "wf-1/run-1/7",
	};
}

describe("WorkerSession", () => {
	it("carries configured task queue, identity, and activity names", async () => {
		const session = new FakeSession();
		const config: WorkerConfig = {
			endpoint: "127.0.0.1:50051",
			namespaces: ["payments"],
			subject: "worker",
			taskQueue: "payments",
			identity: "worker-a",
			maxConcurrency: 2,
			reconnect: { initialDelayMs: 100, maxDelayMs: 1_000, maxAttempts: 5 },
		};

		await session.handshake(config);
		await session.register({
			activityTypes: ["charge", "refund"],
			activities: [],
		});

		expect(session.handshakes).toHaveLength(1);
		expect(session.handshakes[0]?.taskQueue).toBe("payments");
		expect(session.handshakes[0]?.identity).toBe("worker-a");
		expect(session.registrations[0]?.activityTypes).toEqual([
			"charge",
			"refund",
		]);
	});

	it("preserves payload content type on encode/decode", () => {
		const encoded = encodePayload({
			contentType: "application/json",
			bytes: new Uint8Array([123, 125]),
		});

		expect(decodePayload(encoded)).toEqual({
			contentType: "application/json",
			bytes: new Uint8Array([123, 125]),
		});
	});
});

describe("decodeTask", () => {
	it("decodes every field the server stamps, including the fencing identity", () => {
		const task = decodeTask(wireTask());

		expect(task).toEqual({
			workflowId: "wf-1",
			activityId: "7",
			activityType: "charge",
			input: {
				contentType: "application/json",
				bytes: new Uint8Array([123, 125]),
			},
			attempt: 3,
			runId: "run-1",
			completionToken: "generation-9",
			idempotencyKey: "wf-1/run-1/7",
			labels: { brief: "IP-001" },
		});
	});

	it("rejects a task whose attempt is missing or zero", () => {
		// proto3's zero default means the producer failed to stamp it; a
		// consumer that defaulted it would silently mis-report retry counts.
		const { attempt: _attempt, ...missing } = wireTask();

		expect(() => decodeTask(missing as WireActivityTask)).toThrow(
			"activity task attempt is missing or zero",
		);
		expect(() => decodeTask({ ...wireTask(), attempt: 0 })).toThrow(
			"activity task attempt is missing or zero",
		);
	});

	it("rejects a task with no run_id rather than defaulting it", () => {
		// A missing run cannot produce a run-scoped completion, and the server
		// refuses every result lacking one ("activity result run id is
		// missing"). Refusing at decode surfaces the incompatibility at the
		// boundary instead of as silently discarded work.
		expect(() => decodeTask({ ...wireTask(), runId: undefined })).toThrow(
			"activity task is missing run_id",
		);
		expect(() =>
			decodeTask({ ...wireTask(), runId: { uuid: "" } }),
		).toThrow("activity task is missing run_id");
	});

	it("rejects a task with an empty completion_token", () => {
		// Empty is NOT "the current generation": it identifies a pre-fencing
		// server whose completions the fence rejects unconditionally.
		expect(() =>
			decodeTask({ ...wireTask(), completionToken: "" }),
		).toThrow("activity task is missing completion_token");
	});

	it("rejects a task with an empty idempotency_key", () => {
		expect(() =>
			decodeTask({ ...wireTask(), idempotencyKey: "" }),
		).toThrow("activity task is missing idempotency_key");
	});

	it("rejects the other malformed shapes rather than defaulting them", () => {
		expect(() =>
			decodeTask({ ...wireTask(), workflowId: undefined }),
		).toThrow("activity task is missing workflow_id");
		expect(() =>
			decodeTask({ ...wireTask(), workflowId: { uuid: "" } }),
		).toThrow("activity task is missing workflow_id");
		expect(() =>
			decodeTask({ ...wireTask(), activityId: undefined }),
		).toThrow("activity task is missing activity_id");
		expect(() =>
			decodeTask({ ...wireTask(), activityType: "" }),
		).toThrow("activity task is missing activity_type");
		expect(() => decodeTask({ ...wireTask(), input: undefined })).toThrow(
			"activity task is missing input payload",
		);
	});

	it("copies labels rather than aliasing the wire map", () => {
		const wire = wireTask();
		const task = decodeTask(wire);
		wire.labels.brief = "mutated";

		expect(task.labels).toEqual({ brief: "IP-001" });
	});
});

describe("resolveNode", () => {
	it("honours an explicitly configured node", () => {
		expect(resolveNode({ ...sessionConfig(), node: "syd-1" })).toBe("syd-1");
	});

	it("treats an explicit empty node as 'no locality', not as unset", () => {
		// An empty node means the worker is reachable only for unpinned
		// dispatches. Falling back to the hostname here would silently pin a
		// worker the operator deliberately left unpinned.
		expect(resolveNode({ ...sessionConfig(), node: "" })).toBe("");
	});

	it("defaults an absent node to the machine hostname", () => {
		const { node: _node, ...withoutNode } = { ...sessionConfig(), node: "x" };
		const resolved = resolveNode(withoutNode as WorkerConfig);

		expect(resolved.length).toBeGreaterThan(0);
	});
});

describe("workerAuthMetadata", () => {
	it("comma-joins the namespace SET and carries the subject", () => {
		// The server reads both BEFORE the first frame, so they cannot travel
		// in the registration message.
		const metadata = workerAuthMetadata({
			namespaces: ["payments", "ledger"],
			subject: "worker",
		});

		expect(metadata.get("x-aion-namespaces")).toEqual(["payments,ledger"]);
		expect(metadata.get("x-aion-subject")).toEqual(["worker"]);
	});
});

class RecordingStream implements AsyncIterable<ServerToWorker> {
	public readonly messages: WorkerToServer[] = [];
	public endCalls = 0;

	public write(
		message: WorkerToServer,
		callback?: (error?: Error | null) => void,
	): boolean {
		this.messages.push(message);
		callback?.();
		return true;
	}

	public end(): void {
		this.endCalls += 1;
		this.messages.push({});
	}

	public on(): this {
		return this;
	}

	public async *[Symbol.asyncIterator](): AsyncIterableIterator<ServerToWorker> {
		const message = undefined as ServerToWorker | undefined;
		if (message !== undefined) {
			yield message;
		}
	}
}

/**
 * Scripted duplex stream for the ack-contract tests: serves the given
 * response frames in order, then ends (or hangs forever when
 * `hangAfterFrames` is set, for ack/write deadline coverage).
 */
class ScriptedStream implements AsyncIterable<ServerToWorker> {
	public readonly messages: WorkerToServer[] = [];
	public endCalls = 0;

	public constructor(
		private readonly frames: ServerToWorker[] = [],
		private readonly options: {
			readonly hangAfterFrames?: boolean;
			readonly neverFlushWrites?: boolean;
		} = {},
	) {}

	public write(
		message: WorkerToServer,
		callback?: (error?: Error | null) => void,
	): boolean {
		this.messages.push(message);
		if (this.options.neverFlushWrites !== true) {
			callback?.();
		}
		return true;
	}

	public end(): void {
		this.endCalls += 1;
	}

	public on(): this {
		return this;
	}

	public async *[Symbol.asyncIterator](): AsyncIterableIterator<ServerToWorker> {
		for (const frame of this.frames) {
			yield frame;
		}
		if (this.options.hangAfterFrames === true) {
			await new Promise<never>(() => undefined);
		}
	}
}

function sessionConfig(): WorkerConfig {
	return {
		endpoint: "127.0.0.1:50051",
		namespaces: ["payments"],
		subject: "worker",
		taskQueue: "charges",
		node: "syd-1",
		identity: "worker-build-7",
		maxConcurrency: 2,
		reconnect: { initialDelayMs: 5, maxDelayMs: 20, maxAttempts: 3 },
	};
}

function sessionFor(stream: AsyncIterable<ServerToWorker>): GrpcWorkerSession {
	return new GrpcWorkerSession(
		{ streamWorker: () => stream as never },
		{ namespaces: ["payments"], subject: "worker" },
	);
}

function advertisement(
	overrides: Partial<WorkerAdvertisement> = {},
): WorkerAdvertisement {
	return { activityTypes: ["charge"], activities: [], ...overrides };
}

function registerAckFrame(): ServerToWorker {
	return {
		registerAck: {
			workerId: 7n,
			namespace: "payments,ledger",
			heartbeatWindowMs: 30_000n,
		},
	};
}

describe("GrpcWorkerSession registration", () => {
	it("populates ALL SIX RegisterWorker fields as disjoint dimensions", async () => {
		// The defect this pins: the SDK used to send `namespace: taskQueue` —
		// stuffing the pool selector into a correctness boundary, in a field
		// that no longer exists. namespaces / task_queue / node are disjoint.
		const stream = new ScriptedStream([registerAckFrame()]);
		const session = sessionFor(stream);
		await session.handshake({
			...sessionConfig(),
			namespaces: ["payments", "ledger"],
		});

		await session.register({
			activityTypes: ["charge", "refund"],
			activities: [
				{ name: "charge", input: true, output: { type: "object" } },
				{ name: "refund", input: true, output: { type: "object" } },
			],
		});

		expect(stream.messages[0]?.register).toEqual({
			namespaces: ["payments", "ledger"],
			activityTypes: ["charge", "refund"],
			taskQueue: "charges",
			node: "syd-1",
			activities: [
				{
					name: "charge",
					inputSchemaJson: "true",
					outputSchemaJson: '{"type":"object"}',
				},
				{
					name: "refund",
					inputSchemaJson: "true",
					outputSchemaJson: '{"type":"object"}',
				},
			],
			identity: "worker-build-7",
		});
	});

	it("sends an empty activities list when the worker commits to no contract", async () => {
		const stream = new ScriptedStream([registerAckFrame()]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());

		await session.register(advertisement());

		expect(stream.messages[0]?.register?.activities).toEqual([]);
	});

	it("refuses a partial action surface before it reaches the wire", async () => {
		const stream = new ScriptedStream([registerAckFrame()]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());

		await expect(
			session.register({
				activityTypes: ["charge", "refund"],
				activities: [
					{ name: "charge", input: true, output: { type: "object" } },
				],
			}),
		).rejects.toThrow("worker advertises a partial typed action surface");
		expect(stream.messages).toEqual([]);
	});

	it("refuses contracts for activities the worker does not serve", async () => {
		const stream = new ScriptedStream([registerAckFrame()]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());

		await expect(
			session.register({
				activityTypes: ["charge"],
				activities: [
					{ name: "charge", input: true, output: { type: "object" } },
					{ name: "ghost", input: true, output: { type: "object" } },
				],
			}),
		).rejects.toThrow(
			"worker advertises contracts for unserved activities: ghost",
		);
		expect(stream.messages).toEqual([]);
	});

	it("rejects empty activity registrations", async () => {
		const stream = new RecordingStream();
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());

		await expect(
			session.register({ activityTypes: [], activities: [] }),
		).rejects.toThrow(
			"worker registration must include at least one activity type",
		);
		expect(stream.messages).toEqual([]);
	});

	it("rejects a config whose namespace set is empty or malformed", async () => {
		const session = sessionFor(new RecordingStream());

		await expect(
			session.handshake({ ...sessionConfig(), namespaces: [] }),
		).rejects.toThrow("worker namespaces must include at least one namespace");
		await expect(
			session.handshake({ ...sessionConfig(), namespaces: [""] }),
		).rejects.toThrow("worker namespace must not be empty");
		// The auth metadata is comma-joined, so an embedded comma would split
		// one namespace into two and change the authorized boundary.
		await expect(
			session.handshake({ ...sessionConfig(), namespaces: ["a,b"] }),
		).rejects.toThrow("worker namespace must not contain a comma");
		await expect(
			session.handshake({ ...sessionConfig(), subject: "" }),
		).rejects.toThrow("worker subject must not be empty");
	});

	it("close is idempotent: a double close ends the stream exactly once", async () => {
		// The worker loop's failure path and the worker's abort handler can
		// both close the same session in one shutdown race; the contract is
		// that only the first close ends the stream.
		const stream = new RecordingStream();
		const session = sessionFor(stream);

		await session.close();
		await session.close();
		await session.close();

		expect(stream.endCalls).toBe(1);
	});
});

describe("GrpcWorkerSession result reporting", () => {
	it("echoes run_id and completion_token VERBATIM from task to result", async () => {
		// The two fields the server rejects a result for lacking. The token is
		// opaque proof of an execution generation: echoed byte-for-byte, never
		// parsed or rebuilt.
		const stream = new ScriptedStream([registerAckFrame()]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());
		await session.register(advertisement());

		const task: ActivityTask = decodeTask({
			...wireTask(),
			completionToken: "opaque::token::with::structure",
		});
		await session.reportResult(completionIdentity(task), {
			contentType: "application/json",
			bytes: new Uint8Array([1]),
		});

		expect(stream.messages[1]?.result).toEqual({
			workflowId: { uuid: "wf-1" },
			activityId: { sequencePosition: 7n },
			runId: { uuid: "run-1" },
			completionToken: "opaque::token::with::structure",
			result: { contentType: "application/json", bytes: new Uint8Array([1]) },
		});
	});

	it("echoes the same identity on a FAILURE result", async () => {
		const stream = new ScriptedStream([registerAckFrame()]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());
		await session.register(advertisement());

		const task = decodeTask(wireTask());
		await session.reportFailure(completionIdentity(task), {
			retryable: true,
			message: "downstream unavailable",
		});

		const result = stream.messages[1]?.result;
		expect(result?.runId).toEqual({ uuid: "run-1" });
		expect(result?.completionToken).toBe("generation-9");
		expect(result?.error).toEqual({
			kind: 1,
			message: "downstream unavailable",
			details: undefined,
		});
	});

	it("refuses to report an outcome with a blank fencing identity", async () => {
		// Defence in depth: decodeTask already refuses such a task, so reaching
		// here means an identity was hand-built. A result the server would
		// silently discard must never leave the process.
		const stream = new ScriptedStream([registerAckFrame()]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());
		await session.register(advertisement());

		await expect(
			session.reportResult(
				{
					workflowId: "wf-1",
					activityId: "7",
					runId: "",
					completionToken: "generation-9",
				},
				{ contentType: "application/json", bytes: new Uint8Array() },
			),
		).rejects.toThrow("activity result runId must not be empty");
		await expect(
			session.reportResult(
				{
					workflowId: "wf-1",
					activityId: "7",
					runId: "run-1",
					completionToken: "",
				},
				{ contentType: "application/json", bytes: new Uint8Array() },
			),
		).rejects.toThrow("activity result completionToken must not be empty");
	});
});

describe("GrpcWorkerSession ack contract", () => {
	it("register completes only on RegisterAck and exposes its payload", async () => {
		const stream = new ScriptedStream([registerAckFrame()]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());

		await session.register(advertisement());

		expect(session.registeredInfo).toEqual({
			workerId: 7,
			// Informational echo only — the server's resolved scope, which
			// authorization may DERIVE rather than copy, so it need not equal
			// the configured set and nothing branches on it.
			namespaceEcho: "payments,ledger",
			heartbeatWindowMs: 30_000,
		});
		expect(stream.messages[0]?.register?.activityTypes).toEqual(["charge"]);
	});

	it("does not derive routing from the RegisterAck namespace echo", async () => {
		// The echo differs from the configured namespaces on purpose here: the
		// session must keep registering under its OWN configured set.
		const stream = new ScriptedStream([
			{
				registerAck: {
					workerId: 1n,
					namespace: "something-else-entirely",
					heartbeatWindowMs: 1_000n,
				},
			},
		]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());

		await session.register(advertisement());

		expect(session.registeredInfo?.namespaceEcho).toBe(
			"something-else-entirely",
		);
		expect(stream.messages[0]?.register?.namespaces).toEqual(["payments"]);
	});

	it("register times out retryably when the server never acks", async () => {
		const stream = new ScriptedStream([], { hangAfterFrames: true });
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());

		await expect(session.register(advertisement())).rejects.toThrow(
			"did not acknowledge registration within 20ms",
		);
	});

	it("register rejects a non-ack first frame as a protocol violation", async () => {
		const stream = new ScriptedStream([{ task: wireTask() }]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());

		await expect(session.register(advertisement())).rejects.toThrow(
			"protocol violation: server sent a non-RegisterAck frame",
		);
	});

	it("register fails retryably when the stream ends before the ack", async () => {
		const stream = new ScriptedStream([]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());

		await expect(session.register(advertisement())).rejects.toThrow(
			"server ended the stream before acknowledging registration",
		);
	});

	it("receiveTasks yields drained and resultAck events", async () => {
		const stream = new ScriptedStream([
			registerAckFrame(),
			{
				resultAck: {
					workflowId: { uuid: "wf-1" },
					activityId: { sequencePosition: 4n },
				},
			},
			{ drain: {} },
		]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());
		await session.register(advertisement());

		const events: WorkerSessionEvent[] = [];
		for await (const event of session.receiveTasks()) {
			events.push(event);
		}

		expect(events).toEqual([
			{ kind: "resultAck", workflowId: "wf-1", activityId: "4" },
			{ kind: "drained" },
			{ kind: "closed" },
		]);
	});

	it("surfaces a pushed task with its full fencing identity", async () => {
		const stream = new ScriptedStream([registerAckFrame(), { task: wireTask() }]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());
		await session.register(advertisement());

		const events: WorkerSessionEvent[] = [];
		for await (const event of session.receiveTasks()) {
			events.push(event);
		}

		const first = events[0];
		expect(first?.kind).toBe("task");
		if (first?.kind === "task") {
			expect(first.task.runId).toBe("run-1");
			expect(first.task.completionToken).toBe("generation-9");
			expect(first.task.idempotencyKey).toBe("wf-1/run-1/7");
			expect(first.task.labels).toEqual({ brief: "IP-001" });
		}
	});

	it("throws on a RegisterAck arriving mid-stream", async () => {
		const stream = new ScriptedStream([registerAckFrame(), registerAckFrame()]);
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());
		await session.register(advertisement());

		const drain = async (): Promise<void> => {
			for await (const event of session.receiveTasks()) {
				void event;
			}
		};
		await expect(drain()).rejects.toThrow(
			"protocol violation: RegisterAck received after registration completed",
		);
	});

	it("write rejects at the deadline on a never-flushing stream", async () => {
		const stream = new ScriptedStream([registerAckFrame()], {
			neverFlushWrites: true,
		});
		const session = sessionFor(stream);
		await session.handshake(sessionConfig());

		await expect(
			session.reportResult(
				{
					workflowId: "wf-1",
					activityId: "1",
					runId: "run-1",
					completionToken: "generation-9",
				},
				{ contentType: "application/json", bytes: new Uint8Array() },
			),
		).rejects.toThrow("worker stream send did not complete within 20ms");
	});
});
