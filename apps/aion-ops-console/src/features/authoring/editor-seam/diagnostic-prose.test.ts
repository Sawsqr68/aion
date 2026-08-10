import { describe, expect, test } from 'bun:test';

import { proseSegments, splitDiagnosticProse } from './diagnostic-prose';

describe('diagnostic prose segmentation', () => {
  test('splits headline and remedy at the first em-dash separator', () => {
    const prose = splitDiagnosticProse(
      'unknown after target `shp` — declare the step or reference `ship` instead'
    );
    expect(prose.headline).toEqual([
      { kind: 'text', text: 'unknown after target ' },
      { kind: 'code', text: 'shp' },
    ]);
    expect(prose.remedy).toEqual([
      { kind: 'text', text: 'declare the step or reference ' },
      { kind: 'code', text: 'ship' },
      { kind: 'text', text: ' instead' },
    ]);
  });

  test('a message without the separator degrades to headline only', () => {
    const prose = splitDiagnosticProse('expected step');
    expect(prose.headline).toEqual([{ kind: 'text', text: 'expected step' }]);
    expect(prose.remedy).toBeNull();
  });

  test('later separators stay in the remedy, split happens once', () => {
    const prose = splitDiagnosticProse('first — second — third');
    expect(prose.headline).toEqual([{ kind: 'text', text: 'first' }]);
    expect(prose.remedy).toEqual([{ kind: 'text', text: 'second — third' }]);
  });

  test('an unpaired backtick is literal text, never a parse failure', () => {
    expect(proseSegments('a `dangling quote')).toEqual([
      { kind: 'text', text: 'a `dangling quote' },
    ]);
  });

  test('adjacent code segments keep their order and surrounding text', () => {
    expect(proseSegments('`a``b` end')).toEqual([
      { kind: 'code', text: 'a' },
      { kind: 'code', text: 'b' },
      { kind: 'text', text: ' end' },
    ]);
  });
});
