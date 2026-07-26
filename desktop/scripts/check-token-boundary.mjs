import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const stylesDir = resolve(scriptDir, "../src/styles");
const generated = readFileSync(resolve(stylesDir, "brand.generated.css"), "utf8");
const compatibility = readFileSync(resolve(stylesDir, "tokens.css"), "utf8");
const variablePattern = /--kg-[a-z0-9-]+(?=\s*:)/g;

const generatedNames = new Set(generated.match(variablePattern) ?? []);
const compatibilityNames = new Set(compatibility.match(variablePattern) ?? []);
const overlap = [...compatibilityNames]
  .filter((name) => generatedNames.has(name))
  .sort();

if (overlap.length > 0) {
  console.error(
    `Token boundary violation: ${overlap.join(", ")} exist in both ` +
      "tokens.css and brand.generated.css.",
  );
  process.exit(1);
}

console.log(
  `Token boundary valid: ${generatedNames.size} generated, ` +
    `${compatibilityNames.size} product-only variables.`,
);
