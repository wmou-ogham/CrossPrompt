import assert from 'node:assert/strict';
import { test } from 'node:test';
import MarkdownIt from 'markdown-it';
import { splitFrontmatter } from './frontmatter.js';

const SAMPLE = `---
name: porting-session-hygiene
description: Use for any long, iterative hands-on session building/patching a third-party codebase.
---

# Porting / build session hygiene

Keep the tree clean.
`;

test('unsplit YAML header becomes a setext heading in markdown-it', () => {
  const html = new MarkdownIt().render(SAMPLE);
  assert.match(html, /<h2>/);
  assert.match(html, /name: porting-session-hygiene/);
});

test('parses a standard SKILL.md header without swallowing the body', () => {
  const parsed = splitFrontmatter(SAMPLE);
  assert.equal(parsed.fields?.[0]?.key, 'name');
  assert.equal(parsed.fields?.[0]?.value, 'porting-session-hygiene');
  assert.equal(
    parsed.fields?.[1]?.value,
    'Use for any long, iterative hands-on session building/patching a third-party codebase.'
  );
  assert.match(parsed.body, /^# Porting \/ build session hygiene/m);
  assert.equal(parsed.body.includes('name: porting-session-hygiene'), false);
});

test('split body renders as a real markdown heading', () => {
  const html = new MarkdownIt().render(splitFrontmatter(SAMPLE).body);
  assert.match(html, /<h1>/);
  assert.doesNotMatch(html, /<h2>/);
  assert.doesNotMatch(html, /name: porting-session-hygiene/);
});

test('folds YAML >- descriptions used by Cursor skills', () => {
  const source = `---
name: create-skill
description: >-
  Create Cursor Agent Skills. Use when authoring a new skill or asking about
  SKILL.md structure.
disable-model-invocation: true
---

# Creating Skills
`;
  const parsed = splitFrontmatter(source);
  assert.equal(parsed.fields?.[0]?.value, 'create-skill');
  assert.equal(
    parsed.fields?.[1]?.value,
    'Create Cursor Agent Skills. Use when authoring a new skill or asking about SKILL.md structure.'
  );
  assert.equal(parsed.fields?.[2]?.key, 'disable-model-invocation');
  assert.equal(parsed.fields?.[2]?.value, 'true');
  assert.match(parsed.body, /^# Creating Skills/m);
});

test('keeps literal block scalars and quoted values', () => {
  const source = `---
name: "quoted-name"
description: |
  line one
  line two
---

Body
`;
  const parsed = splitFrontmatter(source);
  assert.equal(parsed.fields?.[0]?.value, 'quoted-name');
  assert.equal(parsed.fields?.[1]?.value, 'line one\nline two');
  assert.equal(parsed.body.trim(), 'Body');
});

test('leaves Markdown without a closed header unchanged', () => {
  const source = `---

This is only a thematic break, not a skill header.

# Title
`;
  const parsed = splitFrontmatter(source);
  assert.equal(parsed.fields, null);
  assert.equal(parsed.body, source);
});

test('ignores a later horizontal rule in the body', () => {
  const source = `# Title

---

More text
`;
  const parsed = splitFrontmatter(source);
  assert.equal(parsed.raw, null);
  assert.equal(parsed.body, source);
});
