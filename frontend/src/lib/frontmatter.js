/**
 * Split SKILL.md-style YAML frontmatter from Markdown.
 *
 * markdown-it treats a closing `---` as a setext heading underline, so
 * `name` / `description` would render as a giant H2 unless stripped first.
 *
 * @param {string} content
 * @returns {{ fields: Array<{key: string, value: string}> | null, raw: string | null, body: string }}
 */
export function splitFrontmatter(content) {
  const source = content ?? '';
  const match = source.match(/^(?:\uFEFF)?---[ \t]*\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n|$)/);
  if (!match) {
    return { fields: null, raw: null, body: source };
  }
  return {
    fields: parseYamlMapping(match[1]),
    raw: match[1],
    body: source.slice(match[0].length)
  };
}

/**
 * Parse a flat YAML mapping used by Agent Skill headers.
 * Supports plain scalars, quotes, and `|` / `>` block scalars (with `+` / `-`).
 *
 * @param {string} raw
 * @returns {Array<{key: string, value: string}>}
 */
export function parseYamlMapping(raw) {
  const lines = (raw ?? '').split(/\r?\n/);
  /** @type {Array<{key: string, value: string}>} */
  const fields = [];
  let index = 0;
  while (index < lines.length) {
    const line = lines[index];
    if (!line.trim() || /^\s*#/.test(line) || /^\s/.test(line)) {
      index += 1;
      continue;
    }
    const keyed = line.match(/^([A-Za-z0-9][A-Za-z0-9_-]*)\s*:\s*(.*?)\s*$/);
    if (!keyed) {
      index += 1;
      continue;
    }
    const key = keyed[1];
    const rest = keyed[2];
    const block = rest.match(/^([>|])([+-])?$/);
    if (block) {
      const { collected, nextIndex } = collectBlock(lines, index + 1);
      index = nextIndex;
      fields.push({
        key,
        value: block[1] === '|' ? collected.join('\n') : foldYamlBlock(collected)
      });
      continue;
    }
    fields.push({ key, value: unquoteYaml(rest) });
    index += 1;
  }
  return fields;
}

/**
 * @param {string[]} lines
 * @param {number} startIndex
 */
function collectBlock(lines, startIndex) {
  /** @type {string[]} */
  const collected = [];
  let index = startIndex;
  /** @type {number | null} */
  let indent = null;
  while (index < lines.length) {
    const next = lines[index];
    if (next.trim() === '') {
      collected.push('');
      index += 1;
      continue;
    }
    const leading = (next.match(/^(\s*)/) || ['', ''])[1].length;
    if (leading === 0) break;
    if (indent === null) indent = leading;
    collected.push(next.slice(Math.min(indent, leading)));
    index += 1;
  }
  while (collected.length && collected[0] === '') collected.shift();
  while (collected.length && collected[collected.length - 1] === '') collected.pop();
  return { collected, nextIndex: index };
}

/** @param {string[]} lines */
function foldYamlBlock(lines) {
  /** @type {string[]} */
  const paragraphs = [];
  /** @type {string[]} */
  let current = [];
  for (const line of lines) {
    if (line === '') {
      if (current.length) {
        paragraphs.push(current.join(' '));
        current = [];
      }
    } else {
      current.push(line.replace(/\s+$/, ''));
    }
  }
  if (current.length) paragraphs.push(current.join(' '));
  return paragraphs.join('\n\n').trim();
}

/** @param {string} value */
function unquoteYaml(value) {
  if (!value) return '';
  if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) {
    return value.slice(1, -1).replace(/\\n/g, '\n').replace(/\\"/g, '"').replace(/\\\\/g, '\\');
  }
  if (value.length >= 2 && value.startsWith("'") && value.endsWith("'")) {
    return value.slice(1, -1).replace(/''/g, "'");
  }
  return value;
}
