import { readFileSync } from "node:fs";

interface UnicodeData {
  assigned: number[][];
  punctuation: number[][];
  control: number[][];
  cased: number[][];
  caseIgnorable: number[][];
  lowercase: Record<string, string>;
}
const unicode = JSON.parse(
  readFileSync(
    new URL("../../config/unicode-13.json", import.meta.url),
    "utf8",
  ),
) as UnicodeData;
export const UNICODE_VERSION = "13.0.0";
class QualityError extends Error {}
const whitespace = new Set(
  Array.from(
    "\t\n\v\f\r \u0085\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u2028\u2029\u202f\u205f\u3000",
  ),
);
function inside(cp: number, ranges: number[][]): boolean {
  let lo = 0,
    hi = ranges.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >>> 1;
    const range = ranges[mid]!;
    if (cp < range[0]!) hi = mid - 1;
    else if (cp > range[1]!) lo = mid + 1;
    else return true;
  }
  return false;
}
function text(value: unknown): string {
  if (typeof value !== "string")
    throw new QualityError("Text must be a string");
  for (const c of value) {
    const cp = c.codePointAt(0)!;
    if (cp >= 0xd800 && cp <= 0xdfff)
      throw new QualityError("Text contains invalid Unicode");
  }
  if (Buffer.byteLength(value) > 256 * 1024)
    throw new QualityError("Text exceeds the UTF-8 byte limit");
  return value;
}
export function normalize(value: unknown): string[] {
  const input = Array.from(text(value));
  let run = "";
  let nfc = "";
  // Normalization of assigned characters is stable across Unicode versions.
  // Keep characters unassigned in Unicode 13 outside newer normalization rules.
  for (const c of input) {
    if (inside(c.codePointAt(0)!, unicode.assigned)) run += c;
    else {
      nfc += run.normalize("NFC") + c;
      run = "";
    }
  }
  nfc += run.normalize("NFC");
  const chars = Array.from(nfc);
  const lower = chars
    .map((c, i) => {
      if (c === "Σ") {
        let before = i - 1,
          after = i + 1;
        while (
          before >= 0 &&
          inside(chars[before]!.codePointAt(0)!, unicode.caseIgnorable)
        )
          before--;
        while (
          after < chars.length &&
          inside(chars[after]!.codePointAt(0)!, unicode.caseIgnorable)
        )
          after++;
        if (
          before >= 0 &&
          inside(chars[before]!.codePointAt(0)!, unicode.cased) &&
          (after === chars.length ||
            !inside(chars[after]!.codePointAt(0)!, unicode.cased))
        )
          return "ς";
      }
      return unicode.lowercase[String(c.codePointAt(0)!)] ?? c;
    })
    .join("");
  text(lower);
  const tokens: string[] = [];
  let token = "";
  for (const c of lower) {
    if (whitespace.has(c) || inside(c.codePointAt(0)!, unicode.punctuation)) {
      if (token) {
        tokens.push(token);
        token = "";
      }
    } else token += c;
  }
  if (token) tokens.push(token);
  if (tokens.length > 16384)
    throw new QualityError("Text exceeds the token limit");
  return tokens;
}
function object(x: unknown): Record<string, unknown> {
  if (!x || typeof x !== "object" || Array.isArray(x))
    throw new QualityError("Expected object");
  return x as Record<string, unknown>;
}
function identifier(x: unknown): string {
  const v = text(x);
  if (
    !v ||
    v.trim() !== v ||
    Buffer.byteLength(v) > 256 ||
    Array.from(v).some((c) => inside(c.codePointAt(0)!, unicode.control))
  )
    throw new QualityError("Identifier is invalid or too large");
  return v;
}
function distance(a: string[], b: string[]): number {
  if (a.length < b.length) [a, b] = [b, a];
  let prev = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 0; i < a.length; i++) {
    const cur = [i + 1];
    for (let j = 0; j < b.length; j++)
      cur.push(
        Math.min(
          prev[j + 1]! + 1,
          cur[j]! + 1,
          prev[j]! + (a[i] === b[j] ? 0 : 1),
        ),
      );
    prev = cur;
  }
  return prev[b.length]!;
}
interface Case {
  id: string;
  split: string;
  language: string;
  class: string;
  reference: string[];
}
interface Result {
  status: "ok" | "failed";
  text: string | null;
  tokens: string[] | null;
}
interface Score {
  id: string;
  split: string;
  language: string;
  class: string;
  status: string;
  edits: number | null;
  refWords: number;
  hypWords: number | null;
  wer: number | null;
  unwantedText: boolean | null;
}
interface Group {
  split: string;
  language: string;
  class: string;
  caseCount: number;
  scoredCount: number;
  failedCount: number;
  missingCount: number;
  silenceCount: number;
  silenceFailedCount: number;
  unwantedTextCount: number;
  expectedRefWords: number;
  refWords: number;
  edits: number;
  complete: boolean;
  observedWer: number | null;
  wer: number | null;
}
export function evaluate(
  caseInput: unknown,
  resultInput: unknown,
): {
  schemaVersion: 1;
  unicodeVersion: string;
  cases: Score[];
  groups: Group[];
} {
  if (
    !Array.isArray(caseInput) ||
    caseInput.length < 1 ||
    caseInput.length > 500
  )
    throw new QualityError("Case count must be between 1 and 500");
  if (!Array.isArray(resultInput) || resultInput.length > 500)
    throw new QualityError("Result count exceeds its limit or is not a list");
  const cases = new Map<string, Case>();
  const results = new Map<string, Result>();
  for (const raw of caseInput) {
    const c = object(raw);
    if (
      Object.keys(c).some(
        (k) =>
          ![
            "id",
            "language",
            "class",
            "split",
            "reference",
            "audio",
            "tags",
            "source",
          ].includes(k),
      )
    )
      throw new QualityError("Case contains unsupported fields");
    const id = identifier(c.id);
    if (cases.has(id)) throw new QualityError("Duplicate case identifier");
    if (
      typeof c.language !== "string" ||
      !["de", "en"].includes(c.language) ||
      typeof c.class !== "string" ||
      !["clean", "challenging"].includes(c.class) ||
      typeof c.split !== "string" ||
      !["calibration", "acceptance"].includes(c.split)
    )
      throw new QualityError("Case language, class or split is invalid");
    cases.set(id, {
      id,
      language: c.language,
      class: c.class,
      split: c.split,
      reference: normalize(c.reference),
    });
  }
  for (const raw of resultInput) {
    const r = object(raw);
    if (Object.keys(r).sort().join(",") !== "id,status,text")
      throw new QualityError("Result fields must be id, status and text");
    const id = identifier(r.id);
    if (!cases.has(id))
      throw new QualityError("Result identifier is absent from the manifest");
    if (results.has(id)) throw new QualityError("Duplicate result identifier");
    if (
      (r.status !== "ok" && r.status !== "failed") ||
      (r.status === "ok" && r.text === null)
    )
      throw new QualityError("Result status or successful text is invalid");
    const value = r.text === null ? null : text(r.text);
    results.set(id, {
      status: r.status,
      text: value,
      tokens: value === null ? null : normalize(value),
    });
  }
  let cells = 0;
  for (const [id, c] of cases) {
    const r = results.get(id);
    if (r?.status === "ok" && c.reference.length) {
      const size = (c.reference.length + 1) * (r.tokens!.length + 1);
      cells += size;
      if (size > 2_000_000 || cells > 20_000_000)
        throw new QualityError("Word-distance work budget exceeded");
    }
  }
  const scores: Score[] = [];
  const groups = new Map<string, Group>();
  for (const [id, c] of cases) {
    const r = results.get(id);
    const status = r?.status ?? "missing";
    const edits =
      status === "ok" && c.reference.length
        ? distance(c.reference, r!.tokens!)
        : null;
    const unwanted = c.reference.length
      ? false
      : r?.text != null
        ? Array.from(r.text).some((ch) => !whitespace.has(ch))
        : null;
    scores.push({
      id,
      split: c.split,
      language: c.language,
      class: c.class,
      status,
      edits,
      refWords: c.reference.length,
      hypWords: r?.tokens?.length ?? null,
      wer: edits === null ? null : edits / c.reference.length,
      unwantedText: unwanted,
    });
    const key = [c.split, c.language, c.class].join("\0");
    let g = groups.get(key);
    if (!g) {
      g = {
        split: c.split,
        language: c.language,
        class: c.class,
        caseCount: 0,
        scoredCount: 0,
        failedCount: 0,
        missingCount: 0,
        silenceCount: 0,
        silenceFailedCount: 0,
        unwantedTextCount: 0,
        expectedRefWords: 0,
        refWords: 0,
        edits: 0,
        complete: false,
        observedWer: null,
        wer: null,
      };
      groups.set(key, g);
    }
    g.caseCount++;
    g.expectedRefWords += c.reference.length;
    g.failedCount += Number(status !== "ok");
    g.missingCount += Number(status === "missing");
    g.silenceCount += Number(!c.reference.length);
    g.silenceFailedCount += Number(!c.reference.length && status !== "ok");
    g.unwantedTextCount += Number(unwanted === true);
    if (edits !== null) {
      g.scoredCount++;
      g.refWords += c.reference.length;
      g.edits += edits;
    }
  }
  const ordered = [...groups.entries()]
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([, g]) => {
      g.complete = g.failedCount === 0;
      g.observedWer = g.refWords ? g.edits / g.refWords : null;
      g.wer = g.complete ? g.observedWer : null;
      return g;
    });
  return {
    schemaVersion: 1,
    unicodeVersion: UNICODE_VERSION,
    cases: scores,
    groups: ordered,
  };
}
