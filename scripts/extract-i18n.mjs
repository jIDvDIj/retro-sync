#!/usr/bin/env node
/**
 * Audita as chaves i18n: extrai todas as chaves usadas via t("...") e
 * i18nKey="..." em src/ e compara com as definidas no locale base (en).
 *
 * - Chave usada mas não definida  → erro (exit 1); na prática o tsc já pega,
 *   pois t() é estritamente tipado — isto cobre usos fora do type-check.
 * - Chave definida mas nunca usada → aviso (candidata a remoção).
 * - Usos dinâmicos como t(`errors.${code}`) viram um "prefixo dinâmico":
 *   tudo sob o prefixo é considerado em uso.
 *
 * A paridade en ⇄ pt é responsabilidade do scripts/check-i18n.mjs (via tsc).
 *
 * Uso: npm run i18n:extract
 */

import { build } from "esbuild";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// 1. Compila o locale base (TypeScript) em memória e importa o objeto `en`.
const bundle = await build({
  entryPoints: [join(root, "src/i18n/locales/en/index.ts")],
  bundle: true,
  format: "esm",
  write: false,
  logLevel: "silent",
});
const code = Buffer.from(bundle.outputFiles[0].contents).toString("base64");
const { en } = await import(`data:text/javascript;base64,${code}`);

const defined = new Set();
(function flatten(obj, prefix) {
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === "object") flatten(v, key);
    else defined.add(key);
  }
})(en, "");

// Sufixos de plural do i18next: t("chave") é atendido por "chave_one"/"chave_other".
const PLURAL = /_(zero|one|two|few|many|other)$/;
const pluralBases = new Set(
  [...defined].filter((k) => PLURAL.test(k)).map((k) => k.replace(PLURAL, "")),
);

// 2. Varre src/ (menos src/i18n/) atrás de chaves usadas no código.
const used = new Map(); // chave -> arquivos que a usam
const dynamicPrefixes = new Set();

// Uso direto: t("chave") / i18nKey="chave" — base do relatório de "faltando".
const STATIC_RE = /\b(?:t\(|i18nKey=)\s*["']([^"']+)["']/g;
// Uso dinâmico: t(`prefixo.${...}`) — tudo sob o prefixo conta como em uso.
const DYNAMIC_RE = /\bt\(\s*`([^`$]*)\$\{/g;
// Uso indireto: literal pontuado guardado em constante (labelKey: "x.y") e
// passado a t() depois; só conta se bater exatamente com uma chave definida.
const LITERAL_RE = /["'`]([\w-]+(?:\.[\w-]+)+)["'`]/g;

const files = readdirSync(join(root, "src"), { recursive: true })
  .map((f) => String(f).replaceAll("\\", "/"))
  .filter((f) => /\.(ts|tsx)$/.test(f) && !f.startsWith("i18n/"));

const markUsed = (key, file) => {
  if (!used.has(key)) used.set(key, []);
  if (!used.get(key).includes(file)) used.get(key).push(file);
};

for (const file of files) {
  const text = readFileSync(join(root, "src", file), "utf8");
  for (const m of text.matchAll(STATIC_RE)) markUsed(m[1], file);
  for (const m of text.matchAll(DYNAMIC_RE)) dynamicPrefixes.add(m[1]);
  for (const m of text.matchAll(LITERAL_RE)) {
    if (defined.has(m[1]) || pluralBases.has(m[1])) markUsed(m[1], file);
  }
}

// 3. Relatório.
const missing = [...used.keys()].filter((k) => !defined.has(k) && !pluralBases.has(k));
const unused = [...defined].filter(
  (k) =>
    !used.has(k) &&
    !used.has(k.replace(PLURAL, "")) &&
    ![...dynamicPrefixes].some((p) => k.startsWith(p)),
);

console.log(`[i18n] ${defined.size} chaves definidas (en), ${used.size} usadas em src/.`);
if (dynamicPrefixes.size > 0) {
  console.log(
    `[i18n] Prefixos dinâmicos detectados: ${[...dynamicPrefixes].map((p) => `${p}*`).join(", ")}`,
  );
}

if (unused.length > 0) {
  console.log(`\n[i18n] ${unused.length} chave(s) definida(s) mas nunca usada(s):`);
  for (const k of unused.sort()) console.log(`  - ${k}`);
}

if (missing.length > 0) {
  console.error(`\n[i18n] ERRO: chave(s) usada(s) mas sem definição no locale en:`);
  for (const k of missing.sort()) console.error(`  - ${k}  (${used.get(k).join(", ")})`);
  process.exit(1);
}

console.log("\n[i18n] OK — nenhuma chave usada sem definição.");
