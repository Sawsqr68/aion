import * as grpc from "@grpc/grpc-js";
import type { ServerToWorker, WorkerToServer } from "./proto/worker.js";
import { WorkerProtocolClient as GeneratedWorkerProtocolClient } from "./proto/worker.js";

/**
 * Authorization facts the server reads from gRPC metadata BEFORE the first
 * frame of the stream, in `WorkerGrpcService::stream_worker`
 * (`crates/aion-server/src/api/worker_grpc.rs`). Because they are read ahead of
 * `RegisterWorker`, they cannot be carried in the registration frame: a stream
 * opened without them is rejected (or resolved to a different caller) before
 * the worker's advertisement is ever seen.
 *
 * `namespaces` is the SAME set the registration advertises — the Rust worker
 * pins this in `apply_auth_metadata` — so authorization scope and registration
 * scope can never disagree.
 */
export interface WorkerStreamMetadata {
	/** Correctness/isolation boundaries, sent comma-joined as `x-aion-namespaces`. */
	readonly namespaces: readonly string[];
	/** Auth subject, sent as `x-aion-subject`. */
	readonly subject: string;
}

/**
 * The narrow duplex surface the session needs from a worker-protocol stream.
 *
 * Deliberately smaller than `@grpc/grpc-js`'s `ClientDuplexStream` (which
 * extends `Duplex` and carries the whole Node stream API): the SDK only ever
 * writes frames, ends the stream, and iterates responses, and a small interface
 * is one a test can implement honestly rather than stub with casts. The real
 * `ClientDuplexStream` satisfies it structurally.
 */
export interface WorkerStream extends AsyncIterable<ServerToWorker> {
	write(message: WorkerToServer, callback?: (error?: Error | null) => void): boolean;
	end(): void;
	on(event: "error", listener: (error: Error) => void): this;
	on(event: "end", listener: () => void): this;
	on(event: "data", listener: (message: ServerToWorker) => void): this;
}

/**
 * A connected worker-protocol client. `streamWorker` opens ONE bidirectional
 * stream and must attach {@link WorkerStreamMetadata} to it — the server reads
 * that metadata before the registration frame.
 */
export interface WorkerProtocolTransport {
	streamWorker(metadata: WorkerStreamMetadata): WorkerStream;
}

/**
 * Creates a transport for an endpoint. Injectable so tests (and alternative
 * gRPC runtimes) can supply their own; {@link grpcClientFactory} is the
 * production implementation over `@grpc/grpc-js`.
 */
export interface GrpcClientFactory {
	create(
		endpoint: string,
		credentials: grpc.ChannelCredentials,
		options?: grpc.ChannelOptions,
	): WorkerProtocolTransport;
}

/**
 * Builds the gRPC metadata map the server authorizes the stream from.
 *
 * `namespaces` is comma-joined in advertised order, matching the Rust worker's
 * `apply_auth_metadata`. Exported so a test can assert the exact wire form
 * rather than re-deriving it.
 */
export function workerAuthMetadata(metadata: WorkerStreamMetadata): grpc.Metadata {
	const map = new grpc.Metadata();
	map.set("x-aion-namespaces", metadata.namespaces.join(","));
	map.set("x-aion-subject", metadata.subject);
	return map;
}

/**
 * The production transport: a real `@grpc/grpc-js` client built from the
 * generated `WorkerProtocolClient` in `src/proto/`.
 *
 * This is the default used by {@link connectGrpcWorkerSession} when no factory
 * is supplied, so a worker needs no hand-written wire plumbing to reach a
 * server.
 */
export function grpcClientFactory(): GrpcClientFactory {
	return {
		create(endpoint, credentials, options) {
			const client = new GeneratedWorkerProtocolClient(endpoint, credentials, options);
			return {
				streamWorker(metadata) {
					return client.streamWorker(workerAuthMetadata(metadata));
				},
			};
		},
	};
}
