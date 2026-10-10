// compileProgram(): compile a program's components against one manifest, check the program they make, and return its parts. It composes compile()
// and checkProgram(), so what is pinned is the composition: the parts are the ones a host assembles by hand, the program check is the existing one
// (never a second set of checks), a failure is an existing diagnostics document, and nothing beyond the compiler's own module is needed.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { test } from "node:test";
import { checkProgram, compile, compileProgram } from "../dist/index.js";
import { packageDir, root } from "./common.mjs";

const slice = join(root, "examples", "slice");
const manifest = readFileSync(join(slice, "components.json"), "utf8");
const model = { manifest, path: "components.json" };
const source = (dir, component) => ({ component, source: readFileSync(join(dir, `${component}.mprx`), "utf8"), path: `${component}.mprx` });
const sliceComponents = ["users", "user-card"].map((component) => source(slice, component));

/** The program a host assembles by hand today: compile() each component, write the template out, group root, templates and model. */
async function byHand(input) {
  const templates = [];
  for (const entry of input.components) {
    const result = await compile({ source: entry.source, path: entry.path, model: { manifest: input.model.manifest, path: input.model.path, component: entry.component } });
    assert.ok(result.template, `${entry.component} compiles`);
    templates.push(JSON.stringify(result.template));
  }
  return { model: input.model.manifest, root: input.root, templates };
}

const errors = (document) => document.diagnostics.filter((diagnostic) => diagnostic.severity === "error");

test("compiles one real program (a composite and its root): the parts are the root, the templates as text, and the manifest's own text", async () => {
  const result = await compileProgram({ model, root: "users", components: sliceComponents });

  assert.ok(result.program, JSON.stringify(result.components));
  assert.deepEqual(Object.keys(result.program).sort(), ["model", "root", "templates"]);
  assert.equal(result.program.root, "users");
  assert.equal(result.program.model, manifest, "the model is the manifest's text, as given");
  assert.equal(result.program.templates.length, 2);
  for (const [index, text] of result.program.templates.entries()) {
    assert.equal(typeof text, "string", "a template is text, as the runtime and the program check take it");
    assert.equal(JSON.parse(text).component, sliceComponents[index].component, "in the order of the components");
  }
  assert.deepEqual(result.components.map(({ component }) => component), ["users", "user-card"]);
  assert.ok(result.components.every(({ diagnostics }) => errors(diagnostics).length === 0));
  assert.deepEqual(result.assembly.diagnostics, []);
  assert.deepEqual(await checkProgram(result.program), result.assembly, "its assembly is checkProgram's document for the parts");
});

test("the parts equal the ones a host assembles by hand from compile()", async () => {
  const input = { model, root: "users", components: sliceComponents };

  assert.deepEqual((await compileProgram(input)).program, await byHand(input));
});

test("the program check is checkProgram's own: for every committed check-program case a program of sources, its document is the one `mesh check-program` prints", async () => {
  const cases = join(root, "crates", "mesh-cli", "tests", "check-program");
  // The cases that are a program of sources against ONE model: not those with a model of their own for the check (broken-model), for one template
  // (fingerprint-mismatch), or with a template written as JSON (malformed-template, unsupported-version).
  const names = ["composite-children", "composite-event", "cycle", "duplicate-template", "missing-root", "unbound-scope-name", "unsound-binding", "valid"];

  for (const name of names) {
    const dir = join(cases, name);
    const shared = join(cases, "components.json");
    const spec = JSON.parse(readFileSync(join(dir, "program.json"), "utf8"));
    const expected = JSON.parse(readFileSync(join(dir, "expected.json"), "utf8"));
    const components = spec.templates.map((file) => {
      assert.ok(typeof file === "string" && file.endsWith(".mprx") && !spec.checkModel, `${name} is a program of sources`);
      return source(dir, file.slice(0, -".mprx".length));
    });
    const result = await compileProgram({ model: { manifest: readFileSync(shared, "utf8"), path: "components.json" }, root: spec.root, components });

    assert.deepEqual(result.assembly, expected, name);
    assert.equal(result.program !== undefined, expected.diagnostics.length === 0, `${name}: a program exactly when the check finds nothing`);
  }
});

test("a malformed program is the program check's refusal: its document, and no program", async () => {
  const withoutRoot = await compileProgram({ model, root: "users", components: [sliceComponents[1]] });

  assert.equal(withoutRoot.program, undefined);
  assert.deepEqual(withoutRoot.assembly.diagnostics.map(({ code }) => code), ["assembly-missing-root"]);
  assert.deepEqual(withoutRoot.assembly, await checkProgram(await byHand({ model, root: "users", components: [sliceComponents[1]] })), "the same document as for the same parts by hand");

  const twice = await compileProgram({ model, root: "users", components: [...sliceComponents, sliceComponents[1]] });

  assert.equal(twice.program, undefined);
  assert.deepEqual(twice.assembly.diagnostics.map(({ code }) => code), ["assembly-duplicate-template"]);

  const empty = await compileProgram({ model, root: "users", components: [] });

  assert.equal(empty.program, undefined);
  assert.deepEqual(empty.assembly.diagnostics.map(({ code }) => code), ["assembly-missing-root"]);
});

test("a component that does not compile is compile()'s own document; every component is still compiled, and nothing is assembled", async () => {
  const broken = (entry) => ({ ...entry, source: "<page title={title}>\n  <text>x</heading>\n</page>" });
  const brokenCard = await compileProgram({ model, root: "users", components: [sliceComponents[0], broken(sliceComponents[1])] });

  assert.equal(brokenCard.program, undefined);
  assert.equal(brokenCard.assembly, undefined, "there are no templates to check");
  assert.deepEqual(brokenCard.components[1].diagnostics, (await compile({ source: broken(sliceComponents[1]).source, path: "user-card.mprx", model: { manifest, path: "components.json", component: "user-card" } })).diagnostics);
  assert.ok(errors(brokenCard.components[1].diagnostics).length > 0);
  assert.equal(errors(brokenCard.components[0].diagnostics).length, 0, "the other component is still reported");

  const both = await compileProgram({ model, root: "users", components: sliceComponents.map(broken) });

  assert.deepEqual(both.components.map(({ diagnostics }) => errors(diagnostics).length > 0), [true, true], "one run reports every component's errors, not the first's");
});

test("a warning does not stop the program, and is reported with the component", async () => {
  const warned = { ...sliceComponents[0], source: sliceComponents[0].source.replace("<page title={title}>", "<page title={title} title={title}>") };
  const direct = await compile({ source: warned.source, path: warned.path, model: { manifest, path: "components.json", component: "users" } });

  assert.ok(direct.template && direct.diagnostics.diagnostics.some(({ severity }) => severity === "warning"), "compile() accepts it, with a warning");

  const result = await compileProgram({ model, root: "users", components: [warned, sliceComponents[1]] });

  assert.ok(result.program, "the program is made");
  assert.deepEqual(result.components[0].diagnostics, direct.diagnostics);
});

test("compileProgram() refuses arguments of the wrong type", async () => {
  const entry = sliceComponents[0];

  await assert.rejects(compileProgram(null), TypeError);
  await assert.rejects(compileProgram({ root: "users", components: [] }), TypeError);
  await assert.rejects(compileProgram({ model: { manifest, path: "m" }, components: [] }), TypeError);
  await assert.rejects(compileProgram({ model: { manifest, path: "m" }, root: 1, components: [] }), TypeError);
  await assert.rejects(compileProgram({ model: { manifest: 1, path: "m" }, root: "users", components: [] }), TypeError);
  await assert.rejects(compileProgram({ model, root: "users" }), TypeError);
  await assert.rejects(compileProgram({ model, root: "users", components: "users" }), TypeError);
  await assert.rejects(compileProgram({ model, root: "users", components: [null] }), TypeError);
  await assert.rejects(compileProgram({ model, root: "users", components: [{ ...entry, source: 1 }] }), TypeError);
  await assert.rejects(compileProgram({ model, root: "users", components: [{ ...entry, component: undefined }] }), TypeError);
});

test("it needs no initialization and no module but the compiler's: a fresh process that only imports the package compiles a program", () => {
  const manifestOf = JSON.stringify(manifest);
  const script = `
    import { readFileSync } from "node:fs";
    const { compileProgram } = await import(${JSON.stringify(pathToFileURL(join(packageDir, "dist", "index.js")).href)});
    const read = (file) => readFileSync(${JSON.stringify(slice + "/")} + file, "utf8");
    const result = await compileProgram({
      model: { manifest: ${manifestOf}, path: "components.json" },
      root: "users",
      components: ["users", "user-card"].map((component) => ({ component, source: read(component + ".mprx"), path: component + ".mprx" })),
    });
    process.stdout.write(JSON.stringify({ made: result.program !== undefined, root: result.program?.root, templates: result.program?.templates.length, assembly: result.assembly.diagnostics.length }));
  `;
  const out = JSON.parse(execFileSync(process.execPath, ["--input-type=module", "-e", script], { encoding: "utf8" }));

  assert.deepEqual(out, { made: true, root: "users", templates: 2, assembly: 0 });

  const manifestJson = JSON.parse(readFileSync(join(packageDir, "package.json"), "utf8"));

  assert.equal(manifestJson.dependencies, undefined, "the package depends on nothing: not the runtime, not VALANCE, NEXUS or PORT");
  for (const file of ["index", "program"]) {
    const imports = [...readFileSync(join(packageDir, "dist", `${file}.js`), "utf8").replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/(?:from|import\()\s*"([^"]+)"/g)].map((match) => match[1]);

    assert.ok(imports.every((specifier) => specifier.startsWith("./") || specifier.startsWith("node:")), `${file}.js imports only the package's own modules: ${imports}`);
  }
});

test("a composite the manifest does not declare gets its contract from the templates, and a declared one is used as written", async () => {
  const declared = JSON.parse(manifest);
  const { users, "user-card": card, ...rest } = declared.components;
  const undeclared = JSON.stringify({ ...declared, components: { ...rest, users } });
  const result = await compileProgram({ model: { manifest: undeclared, path: "components.json" }, root: "users", components: sliceComponents });

  assert.ok(result.program, JSON.stringify(result.components));
  const inferred = JSON.parse(result.program.model).components;
  assert.deepEqual(Object.keys(inferred["user-card"].props).sort(), Object.keys(card.props).sort());
  assert.deepEqual(inferred.users, users, "a declared component is untouched");

  const withCard = await compileProgram({ model, root: "users", components: sliceComponents });
  assert.equal(withCard.program.model, manifest, "with every component declared the model is the manifest's text, as given");
});
