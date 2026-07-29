#!/usr/bin/env node
/**
 * Generates `src/proto/` from the authoritative protobuf contract in
 * `crates/aion-proto-generated/proto/`.
 *
 * Why this exists as an importable module rather than an inline npm command:
 * the checked-in stubs drifted from the wire contract for months without
 * anyone noticing, because the only regeneration recipe was an inline `protoc`
 * invocation pointing at a directory (`crates/aion-proto/proto`) that had been
 * deleted, and nothing ever compared the committed output against a fresh
 * generation. `src/proto-contract.test.ts` imports the exports below and byte-
 * compares, so the drift is caught by the ordinary test run rather than by a
 * ritual nobody performs.
 *
 * CLI:
 *   node scripts/generate-proto.mjs           rewrite src/proto/ in place
 *   node scripts/generate-proto.mjs --check   generate to a temp dir and diff;
 *                                             exit non-zero when they differ
 */

import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const PACKAGE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const repoRoot = resolve(PACKAGE_ROOT, "..", "..", "..");

/**
 * The single authoritative contract directory. `crates/aion-proto-generated`
 * owns the `.proto` sources the Rust server and Rust worker are built from, so
 * generating from anywhere else is how the SDKs drift.
 */
export const PROTO_DIR = join(repoRoot, "crates", "aion-proto-generated", "proto");

/** Only the files the worker SDK needs; the engine's other protos are not ours. */
export const PROTO_FILES = ["common.proto", "worker.proto"];

/** Where the generated stubs are committed. Every file here is generated. */
export const OUTPUT_DIR = join(PACKAGE_ROOT, "src", "proto");

/**
 * ts-proto options, held here so the recipe is reviewable in one place.
 *
 * - `esModuleInterop` / `importSuffix=.js`: the package is ESM and compiles
 *   with explicit `.js` specifiers on relative imports.
 * - `outputServices=grpc-js`: emits the `WorkerProtocolClient` constructor the
 *   SDK's default transport wraps. Without it there is no wire client at all.
 * - `forceLong=bigint`: `uint64` fields (worker_id, heartbeat_window_ms,
 *   sequence_position) become `bigint` rather than a lossy `number` or an
 *   extra `long` runtime dependency.
 * - `useExactTypes=false`: keeps generated message types structurally
 *   assignable from plain object literals, which is what the SDK builds.
 */
export const TS_PROTO_OPTIONS = [
	"esModuleInterop=true",
	"outputServices=grpc-js",
	"useExactTypes=false",
	"forceLong=bigint",
	"importSuffix=.js",
].join(",");

/** Runs protoc + ts-proto, writing generated stubs into `destination`. */
export function generateInto(destination) {
	mkdirSync(destination, { recursive: true });
	execFileSync(
		join(PACKAGE_ROOT, "node_modules", ".bin", "grpc_tools_node_protoc"),
		[
			`--plugin=protoc-gen-ts_proto=${join(PACKAGE_ROOT, "node_modules", ".bin", "protoc-gen-ts_proto")}`,
			`--ts_proto_out=${destination}`,
			`--ts_proto_opt=${TS_PROTO_OPTIONS}`,
			"-I",
			PROTO_DIR,
			...PROTO_FILES.map((file) => join(PROTO_DIR, file)),
		],
		{ stdio: ["ignore", "pipe", "pipe"] },
	);
	return destination;
}

/**
 * The `.ts` file names in a directory, sorted. Comparing these sets is what
 * catches a file protoc STARTED or STOPPED emitting, and any hand-written
 * stray that regeneration could never check.
 */
export function generatedFileNames(directory) {
	return readdirSync(directory)
		.filter((name) => name.endsWith(".ts"))
		.sort();
}

/** Generates into a fresh temp directory. The caller owns cleanup. */
export function generateIntoTempDir() {
	return generateInto(mkdtempSync(join(tmpdir(), "aion-worker-proto-")));
}

/**
 * Compares a freshly generated tree against the committed one.
 * Returns `{ missing, unexpected, differing }`, all empty when in sync.
 */
export function compareWithCommitted(fresh) {
	const freshNames = generatedFileNames(fresh);
	const committedNames = generatedFileNames(OUTPUT_DIR);
	const missing = freshNames.filter((name) => !committedNames.includes(name));
	const unexpected = committedNames.filter((name) => !freshNames.includes(name));
	const differing = freshNames
		.filter((name) => committedNames.includes(name))
		.filter(
			(name) =>
				!readFileSync(join(fresh, name)).equals(readFileSync(join(OUTPUT_DIR, name))),
		);
	return { missing, unexpected, differing };
}

function runCli() {
	const checkOnly = process.argv.includes("--check");
	const temporary = generateIntoTempDir();
	try {
		if (checkOnly) {
			const { missing, unexpected, differing } = compareWithCommitted(temporary);
			const problems = [...missing, ...unexpected, ...differing];
			if (problems.length > 0) {
				process.stderr.write(
					`src/proto/ is STALE relative to ${PROTO_DIR}\n` +
						`  differing: ${differing.join(", ") || "none"}\n` +
						`  missing from src/proto/: ${missing.join(", ") || "none"}\n` +
						`  present but not generated: ${unexpected.join(", ") || "none"}\n` +
						"  the SDK is speaking an older wire contract than the server.\n" +
						"  run `npm run generate:proto` and commit the result.\n",
				);
				process.exitCode = 1;
				return;
			}
			process.stdout.write("src/proto/ matches the authoritative proto contract\n");
			return;
		}
		// Replace only after a successful generation, so a failed protoc run
		// never leaves src/proto/ half-written.
		rmSync(OUTPUT_DIR, { recursive: true, force: true });
		mkdirSync(OUTPUT_DIR, { recursive: true });
		for (const name of generatedFileNames(temporary)) {
			writeFileSync(join(OUTPUT_DIR, name), readFileSync(join(temporary, name)));
		}
		process.stdout.write(`regenerated src/proto/ from ${PROTO_DIR}\n`);
	} finally {
		rmSync(temporary, { recursive: true, force: true });
	}
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	runCli();
}
