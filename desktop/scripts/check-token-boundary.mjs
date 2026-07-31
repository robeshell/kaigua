import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const stylesDir = resolve(scriptDir, "../src/styles");
const generated = readFileSync(resolve(stylesDir, "brand.generated.css"), "utf8");
const compatibility = readFileSync(resolve(stylesDir, "tokens.css"), "utf8");
const variablePattern = /--kg-[a-z0-9-]+(?=\s*:)/g;
const numericFontSizePattern = /font-size\s*:\s*[0-9]+(?:\.[0-9]+)?px\b/g;
const arbitraryTextSizePattern = /text-\[[0-9]+(?:\.[0-9]+)?px\]/g;
const sourceDir = resolve(scriptDir, "../src");

const generatedNames = new Set(generated.match(variablePattern) ?? []);
const compatibilityNames = new Set(compatibility.match(variablePattern) ?? []);
const overlap = [...compatibilityNames]
  .filter((name) => generatedNames.has(name))
  .sort();

const sourceFiles = [];
function collectSourceFiles(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) collectSourceFiles(path);
    else if (/\.(css|tsx|ts)$/.test(entry.name) && !path.endsWith("brand.generated.css")) {
      sourceFiles.push(path);
    }
  }
}
collectSourceFiles(sourceDir);

if (overlap.length > 0) {
  console.error(
    `Token boundary violation: ${overlap.join(", ")} exist in both ` +
      "tokens.css and brand.generated.css.",
  );
  process.exit(1);
}

const typographyViolations = sourceFiles.flatMap((path) => {
  const source = readFileSync(path, "utf8");
  return [
    ...(source.match(numericFontSizePattern) ?? []).map((match) => `${path}: ${match}`),
    ...(source.match(arbitraryTextSizePattern) ?? []).map((match) => `${path}: ${match}`),
  ];
});

if (typographyViolations.length > 0) {
  console.error(
    "Typography boundary violation: numeric application font sizes are forbidden:\n" +
      typographyViolations.join("\n"),
  );
  process.exit(1);
}

console.log(
  `Token boundary valid: ${generatedNames.size} generated, ` +
    `${compatibilityNames.size} product-only variables; ` +
    `typography checked in ${sourceFiles.length} application source files.`,
);
