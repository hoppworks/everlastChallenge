/** Bounded static fixture contract; this does not execute scripts or prove native delivery. */
import { JSDOM } from "jsdom";
import { check } from "../../../scripts/tooling/audio-data.ts";
import { record } from "../../../scripts/tooling/io.ts";
export function validate(
  html: string,
  input: unknown,
): { htmlCases: 18; workflows: 20; nativeExecutionPerformed: false } {
  check(Buffer.byteLength(html) <= 128 * 1024, "Fixture exceeds bound");
  const dom = new JSDOM(html); // Default configuration never executes fixture scripts or loads resources.
  try {
    const document = dom.window.document,
      nodes = new Map<string, Element>(),
      cases = new Map<string, string>(),
      labels = new Set<string>();
    let csp: string | undefined;
    for (const element of document.querySelectorAll("*")) {
      const tag = element.tagName.toLowerCase();
      for (const attr of element.attributes) {
        check(!attr.name.startsWith("on"), "Inline event forbidden");
        if (
          ["href", "src", "action", "formaction", "poster"].includes(attr.name)
        )
          check(
            attr.name === "href" && attr.value.startsWith("#"),
            "Resource target forbidden",
          );
      }
      check(
        ![
          "iframe",
          "object",
          "embed",
          "form",
          "base",
          "link",
          "img",
          "video",
          "audio",
        ].includes(tag),
        "Resource element forbidden",
      );
      if (
        tag === "meta" &&
        element.getAttribute("http-equiv")?.toLowerCase() ===
          "content-security-policy"
      ) {
        check(csp === undefined, "Duplicate CSP");
        csp = element.getAttribute("content") ?? "";
      }
      if (tag === "label") labels.add(element.getAttribute("for") ?? "");
      if (element.hasAttribute("id")) {
        check(!nodes.has(element.id), "Duplicate DOM id");
        nodes.set(element.id, element);
      }
      const cid = element.getAttribute("data-case");
      if (cid !== null) {
        check(!cases.has(cid) && !!element.id, "Duplicate or unnamed case");
        cases.set(cid, element.id);
      }
    }
    check(csp !== undefined, "CSP missing");
    const directives = new Map<string, string>();
    for (const part of csp.split(";")) {
      const [key, ...values] = part.trim().split(/\s+/);
      if (key) {
        check(!directives.has(key), "Duplicate CSP directive");
        directives.set(key, values.join(" "));
      }
    }
    for (const key of [
      "default-src",
      "connect-src",
      "img-src",
      "font-src",
      "frame-src",
      "object-src",
      "base-uri",
      "form-action",
    ])
      check(directives.get(key) === "'none'", "Resource CSP weakened");
    for (const key of ["script-src", "style-src"])
      check(
        directives.get(key) === "'unsafe-inline'",
        "Local code/style policy differs",
      );
    const expected = new Set([
      "E01",
      "E02",
      "E03",
      "E04",
      "N01",
      "N02",
      "N03",
      "N04",
      "N05",
      "N06",
      "N07",
      "D01",
      "D02",
      "D03",
      "D04",
      "L01",
      "L02",
      "S01",
    ]);
    check(
      cases.size === expected.size &&
        [...cases.keys()].every((x) => expected.has(x)),
      "HTML cases differ",
    );
    for (const id of cases.values()) {
      const el = nodes.get(id)!;
      if (["INPUT", "TEXTAREA"].includes(el.tagName))
        check(labels.has(id), "Visible field label missing");
      if (["textbox", "listbox"].includes(el.getAttribute("role") ?? ""))
        check(
          nodes.has(el.getAttribute("aria-labelledby") ?? ""),
          "Accessible label missing",
        );
    }
    const aria = nodes.get("aria-input"),
      password = nodes.get("password-input");
    check(
      aria?.getAttribute("aria-readonly") === "true" &&
        !aria.hasAttribute("readonly") &&
        !aria.hasAttribute("disabled"),
      "ARIA-only case disguised",
    );
    check(
      password?.getAttribute("type") === "password" &&
        !password.hasAttribute("value"),
      "Password case prefilled",
    );
    const scripts = document.querySelectorAll("script");
    check(scripts.length === 1, "One local script required");
    check(
      !/\b(fetch|XMLHttpRequest|WebSocket|EventSource|sendBeacon|localStorage|sessionStorage|indexedDB|console|eval|Function)\b|\.\s*(value|clipboard)\b/.test(
        scripts[0]!.textContent,
      ),
      "Input reading or telemetry forbidden",
    );
    const matrix = record(input);
    check(
      matrix.schemaVersion === 1 &&
        matrix.nativeExecutionPerformed === false &&
        matrix.qualified === false &&
        matrix.delayMillis === 5000,
      "Prospective scope changed",
    );
    check(
      Array.isArray(matrix.cases) && matrix.cases.length === 20,
      "Workflow count differs",
    );
    const rows = matrix.cases.map(record);
    const ids = new Set(rows.map((row) => row.id));
    check(
      ids.size === 20 &&
        [...expected, "X01", "A21a-transport"].every((id) => ids.has(id)),
      "Workflow identities differ",
    );
    for (const row of rows)
      check(
        row.status === "not-run" &&
          row.observations === null &&
          typeof row.element === "string" &&
          nodes.has(row.element) &&
          Array.isArray(row.requirements) &&
          row.requirements.length &&
          Array.isArray(row.actions) &&
          row.actions.length &&
          !!row.expected,
        "Workflow has claims or missing target",
      );
    return { htmlCases: 18, workflows: 20, nativeExecutionPerformed: false };
  } finally {
    dom.window.close();
  }
}
