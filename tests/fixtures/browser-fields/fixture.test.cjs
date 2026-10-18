const fs = require("node:fs");
const vm = require("node:vm");
const test = require("node:test");
const assert = require("node:assert/strict");
const html = fs.readFileSync(__dirname + "/index.html", "utf8");
const source = html.match(/<script id="fixture-code">([\s\S]*?)<\/script>/)[1];

function setup() {
  const nodes = new Map();
  let active = null;
  let timer = null;
  let requestedDelay = null;
  class Element {
    constructor(tag, id) {
      this.tagName = tag;
      this.id = id;
      this.attrs = {};
      this.dataset = {};
      this.children = [];
      this.textContent = "";
      this.listeners = {};
      if (id) nodes.set(id, this);
    }
    setAttribute(name, value) { this.attrs[name] = value; }
    removeAttribute(name) { delete this.attrs[name]; }
    addEventListener(name, callback) { this.listeners[name] = callback; }
    focus() { active = this; }
    replaceWith(next) { nodes.set(this.id, next); }
    replaceChildren(...next) {
      for (const child of this.children) nodes.delete(child.id);
      this.children = next;
      for (const child of next) nodes.set(child.id, child);
    }
    get value() { throw Error("Fixture must not read input/password values"); }
  }
  for (const id of ["replace-input", "lock-input", "disable-input", "late-container", "synthetic-option", "action-status"])
    new Element("input", id);
  const buttons = [...html.matchAll(/data-action="([^"]+)"/g)].map(m => {
    const el = new Element("button");
    el.dataset.action = m[1];
    return el;
  });
  const document = {
    getElementById: id => nodes.get(id),
    createElement: tag => new Element(tag),
    querySelectorAll: () => buttons
  };
  vm.runInNewContext(source, {
    document,
    setTimeout: (callback, delay) => { timer = callback; requestedDelay = delay; return 1; },
    clearTimeout: () => { timer = null; }
  });
  return {
    nodes,
    get active() { return active; },
    get delay() { return requestedDelay; },
    click: action => buttons.find(b => b.dataset.action === action).listeners.click(),
    fire: () => { const callback = timer; timer = null; if (callback) callback(); }
  };
}

test("D01 delays replacement, retains public identity attributes and focuses a fresh empty node", () => {
  const page = setup();
  const old = page.nodes.get("replace-input");
  page.click("replace");
  assert.equal(page.nodes.get("replace-input"), old);
  assert.equal(page.active, old);
  assert.equal(page.delay, 5000);
  page.fire();
  const next = page.nodes.get("replace-input");
  assert.notEqual(next, old);
  assert.equal(next.id, "replace-input");
  assert.equal(next.type, "text");
  assert.equal(next.attrs["data-case"], "D01");
  assert.equal(page.active, next);
});

test("D02 retains node/focus and only after five seconds declares readonly", () => {
  const page = setup();
  const field = page.nodes.get("lock-input");
  page.click("lock");
  assert.equal(page.active, field);
  assert.equal(field.attrs.readonly, undefined);
  page.fire();
  assert.equal(page.nodes.get("lock-input"), field);
  assert.equal(field.attrs.readonly, "");
  assert.equal(field.attrs["aria-readonly"], "true");
  assert.equal(page.active, field);
});

test("D03 starts on a non-text container then exposes a fresh conversation field", () => {
  const page = setup();
  const container = page.nodes.get("late-container");
  page.click("late");
  assert.equal(page.active, container);
  assert.equal(page.nodes.has("late-input"), false);
  page.fire();
  const field = page.nodes.get("late-input");
  assert.equal(field.type, "text");
  assert.equal(field.attrs["data-case"], "D03");
  assert.equal(page.active, field);
  assert.equal(container.children.length, 1);
});

test("reset cancels the single pending action and restores fresh dynamic controls", () => {
  const page = setup();
  page.click("replace");
  const original = page.nodes.get("replace-input");
  page.click("lock");
  assert.equal(page.active, original);
  page.click("reset");
  const reset = page.nodes.get("replace-input");
  assert.notEqual(reset, original);
  page.fire();
  assert.equal(page.nodes.get("replace-input"), reset);
  assert.equal(page.nodes.has("late-input"), false);
  assert.equal(page.nodes.get("lock-input").attrs.readonly, undefined);
});

test("synthetic option action only focuses its own non-text option", () => {
  const page = setup();
  page.click("option");
  assert.equal(page.active, page.nodes.get("synthetic-option"));
  assert.equal(page.delay, null);
});

test("D04 exposes a controlled enabled-to-disabled transition without reading its value", () => {
  const page = setup();
  const field = page.nodes.get("disable-input");
  page.click("disable");
  assert.equal(page.active, field);
  assert.equal(field.attrs.disabled, undefined);
  page.fire();
  assert.equal(page.nodes.get("disable-input"), field);
  assert.equal(field.attrs.disabled, "");
});
