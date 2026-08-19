import fs from "node:fs";
import path from "node:path";

const root = process.env.PULSESEEK_VALIDATION_ROOT ?? process.cwd();
const meteringSpecs = [
  "spec/metering-functional-specification.md",
  "spec/metering-dsp-specification.md",
  "spec/metering-architecture-specification.md",
  "spec/metering-validation-specification.md",
];
const requirementPattern = /\b(?:FR|NFR)-[A-Z]+-\d{3}\b/g;
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const fail = (message) => {
  throw new Error(message);
};

const traceability = read("docs/metering-traceability.md");
const requirements = new Set(
  meteringSpecs.flatMap((file) => read(file).match(requirementPattern) ?? []),
);
const productSpec = read("spec/functional-specification.md");
const requiredContractIds = [
  ...productSpec.matchAll(/\b(?:FR-VS|NFR-MT)-\d{3}\b/g),
].map(([id]) => id);
for (const id of requiredContractIds) requirements.add(id);
const allRequirements = [...requirements];
const tracedIds = traceability.match(requirementPattern) ?? [];
const traced = new Set(tracedIds);
const missing = allRequirements.filter((id) => !traced.has(id));
const rowIds = [
  ...traceability.matchAll(/^\| ((?:FR|NFR)-[A-Z]+-\d{3}) \|/gm),
].map(([, id]) => id);
const rows = new Set(rowIds);
const duplicateRowIds = rowIds.filter(
  (id, index) => rowIds.indexOf(id) !== index,
);
const missingRows = allRequirements.filter((id) => !rows.has(id));
if (missing.length > 0) fail(`Missing traceability: ${missing.join(", ")}`);
if (missingRows.length > 0)
  fail(`Missing traceability rows: ${missingRows.join(", ")}`);
if (duplicateRowIds.length > 0)
  fail(
    `Duplicate traceability rows: ${[...new Set(duplicateRowIds)].join(", ")}`,
  );
if (rows.size !== allRequirements.length)
  fail(
    `Expected ${allRequirements.length} explicit traceability rows, found ${rows.size}`,
  );

if (new Set(requiredContractIds).size !== requiredContractIds.length)
  fail("Duplicate product-level requirement IDs");

for (const file of [
  ...meteringSpecs,
  "spec/functional-specification.md",
  "docs/adr/0013-metering-architecture-and-contracts.md",
]) {
  if (!fs.existsSync(path.join(root, file))) fail(`Missing file: ${file}`);
}

const documents = [
  ...meteringSpecs,
  "spec/functional-specification.md",
  "spec/implementation-plan.md",
  "docs/adr/0013-metering-architecture-and-contracts.md",
  "docs/architecture/realtime-metering-engine.md",
  "docs/architecture/metering-event-and-cache-contracts.md",
  "docs/dsp/metering-dsp-algorithms.md",
  "docs/testing/metering-calibration-and-performance.md",
  "docs/metering-traceability.md",
];
const links = documents.flatMap((file) =>
  [
    ...read(file).matchAll(
      /(?<![A-Za-z0-9_])(?:`|\()((?:spec|docs|scripts)\/[^)`\s]+\.md)(?:#[^)`\s]*)?(?:`|\))/g,
    ),
  ].map((match) => match[1]),
);
for (const link of links) {
  if (!fs.existsSync(path.join(root, link)))
    fail(`Broken metering link: ${link}`);
}
const requiredLinks = [
  "spec/metering-functional-specification.md",
  "spec/metering-dsp-specification.md",
  "spec/metering-architecture-specification.md",
  "spec/metering-validation-specification.md",
  "docs/architecture/realtime-metering-engine.md",
  "docs/dsp/metering-dsp-algorithms.md",
  "docs/architecture/metering-event-and-cache-contracts.md",
  "docs/testing/metering-calibration-and-performance.md",
];
for (const link of requiredLinks)
  if (!productSpec.includes(link))
    fail(`Product contract missing link: ${link}`);

if (!read("spec/metering-dsp-specification.md").includes("88.2"))
  fail("DSP rate contract missing 88.2 kHz");
if (!read("spec/metering-validation-specification.md").includes("88.2"))
  fail("Validation rate contract missing 88.2 kHz");
if (!read("spec/metering-validation-specification.md").includes("192"))
  fail("Validation rate contract missing 192 kHz");
const validationSpec = read("spec/metering-validation-specification.md");
for (const tile of [
  "Spectrum",
  "Band Energy",
  "Colored Waveform",
  "Spectrogram",
  "Loudness",
  "True Peak",
  "Stereo",
  "Diagnostics",
]) {
  if (!validationSpec.includes(tile))
    fail(`Canonical default tile missing: ${tile}`);
}
const engineDoc = read("docs/architecture/realtime-metering-engine.md");
if (!engineDoc.includes("SystemMix | InputLoopback | DAWBridge"))
  fail("Supporting source-point vocabulary is stale");
if (!engineDoc.includes("continuous measurements incomplete"))
  fail("Supporting loss semantics are stale");
if (!engineDoc.includes("Visual-only"))
  fail("Visual-only loss semantics missing");
if (
  !read("spec/metering-functional-specification.md").includes(
    "metering-functional-v1",
  )
)
  fail("Functional specification is not frozen");
if (
  !read("docs/architecture/metering-event-and-cache-contracts.md").includes(
    "0013-metering-architecture-and-contracts",
  )
)
  fail("Event/cache decision link missing");
if (
  !read("docs/dsp/metering-dsp-algorithms.md").includes(
    "0013-metering-architecture-and-contracts",
  )
)
  fail("DSP decision link missing");

console.log(
  `Validated ${requirements.size} metering requirements and ${links.length} documentation links.`,
);
