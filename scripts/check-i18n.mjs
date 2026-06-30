#!/usr/bin/env node
/**
 * Valida paridade de chaves i18n via TypeScript.
 *
 * Cada arquivo pt/*.ts é tipado como Localized<typeof ...En>, então qualquer
 * chave faltando ou com tipo errado já é um erro de compilação. Este script
 * executa tsc --noEmit nos arquivos de locale e exibe um resumo legível.
 *
 * Uso: node scripts/check-i18n.mjs
 */

import { execSync } from "child_process";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

try {
  execSync("npx tsc --noEmit --pretty", {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, FORCE_COLOR: "1" },
  });
  console.log("[i18n] Paridade de chaves OK — sem erros de tipo.");
} catch {
  // tsc já imprimiu os erros; apenas sinaliza falha com mensagem clara.
  console.error(
    "\n[i18n] Falha na verificação de paridade." +
      " Verifique as chaves faltando nos arquivos src/i18n/locales/pt/.",
  );
  process.exit(1);
}
