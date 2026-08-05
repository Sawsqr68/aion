import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { assistantContinuePayload, assistantEndPayload } from '../lib/contract';

/**
 * The console kept its own copy of the assistant's signal contract, in a file
 * whose own comment claimed it existed so the UI and the workflow "can only
 * drift in one visible spot". They drifted anyway: the workflow declares
 * `Continuation { message: String, end: Bool }` with NEITHER field optional,
 * while the console sent `{ message }` to continue and `{ end: true }` to
 * finish. Both were refused at the package contract, so the assistant could not
 * be driven from the console at all.
 *
 * A comment cannot force agreement. This can: the expected field list is read
 * off the workflow document at test time, so adding, removing, or renaming a
 * `Continuation` field fails here until both payload builders follow.
 */

/** The workflow document that OWNS this contract. */
const ASSISTANT_AWL = join(import.meta.dir, '../../../../../../examples/assistant/awl/assistant.awl');

/** Fields of an AWL record type, and whether each one may be omitted. */
type Field = { name: string; optional: boolean };

/**
 * Read one `type Name { … }` declaration out of an AWL document.
 *
 * Deliberately not a general AWL parser — it reads exactly the one declaration
 * this contract depends on, and throws rather than returning an empty list if
 * it cannot find it. An empty list would make every assertion below vacuous,
 * which is the failure mode a drift gate must never have.
 */
function readRecordType(source: string, typeName: string): Field[] {
  const opener = new RegExp(`(?:^|\\n)type\\s+${typeName}\\s*\\{`);
  const start = source.search(opener);
  if (start === -1) {
    throw new Error(`no \`type ${typeName}\` declaration in ${ASSISTANT_AWL}`);
  }
  const open = source.indexOf('{', start);
  const close = source.indexOf('}', open);
  if (close === -1) {
    throw new Error(`\`type ${typeName}\` is not closed in ${ASSISTANT_AWL}`);
  }
  const fields: Field[] = [];
  for (const rawLine of source.slice(open + 1, close).split('\n')) {
    const line = rawLine.trim();
    // `///` doc lines attach to the field below them and declare nothing.
    if (line === '' || line.startsWith('//')) {
      continue;
    }
    const match = /^([a-z][a-z0-9_]*)\s*:\s*([^,]+),?$/.exec(line);
    const name = match?.[1];
    const declared = match?.[2];
    if (name === undefined || declared === undefined) {
      throw new Error(`unreadable field line in \`type ${typeName}\`: ${line}`);
    }
    fields.push({ name, optional: declared.trim().endsWith('?') });
  }
  if (fields.length === 0) {
    throw new Error(`\`type ${typeName}\` declares no fields — the gate would be vacuous`);
  }
  return fields;
}

const source = readFileSync(ASSISTANT_AWL, 'utf8');
const continuation = readRecordType(source, 'Continuation');
const required = continuation.filter((field) => !field.optional).map((field) => field.name).sort();

describe('the assistant signal payloads match the workflow that receives them', () => {
  test('the workflow document is the source of the field list', () => {
    // Guards the reader itself: if this ever reads zero required fields, every
    // assertion below would pass against any payload at all.
    expect(required.length).toBeGreaterThan(0);
    expect(required).toEqual(['end', 'message']);
  });

  test('continuing carries every required field', () => {
    expect(Object.keys(assistantContinuePayload('go on')).sort()).toEqual(required);
  });

  test('ending carries every required field', () => {
    expect(Object.keys(assistantEndPayload()).sort()).toEqual(required);
  });

  test('the two payloads differ only in the discriminator and the prompt', () => {
    expect(assistantContinuePayload('go on')).toEqual({ message: 'go on', end: false });
    expect(assistantEndPayload()).toEqual({ message: '', end: true });
  });

  test('a payload missing a required field is what the server refused', () => {
    // The exact shape the console used to send, kept as the negative control so
    // this gate is known to be capable of failing.
    expect(Object.keys({ message: 'go on' }).sort()).not.toEqual(required);
    expect(Object.keys({ end: true }).sort()).not.toEqual(required);
  });
});
