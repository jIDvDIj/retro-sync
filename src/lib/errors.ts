import type { TFunction } from "i18next";
import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import type { AppErrorPayload } from "../types/ipc";

/**
 * Mensagem de erro localizada a partir do payload rejeitado por um comando
 * Tauri. O prefixo é traduzido pelo `code` (enum fechado, ver `error.rs`); o
 * `detail` técnico, vindo do backend, é anexado quando existe. `other` não tem
 * prefixo — mostra-se o `message`/`detail` como veio.
 */
export function translateError(t: TFunction, err: unknown): string {
  const payload = err as Partial<AppErrorPayload>;
  const code = payload?.code;

  if (!code) return payload?.message ?? t("errors.unexpected");
  if (code === "other") return payload.detail || payload.message || t("errors.unexpected");

  const prefix = t(`errors.${code}`);
  return payload.detail ? `${prefix}: ${payload.detail}` : prefix;
}

/** Versão hook: pega o `t` ativo e devolve um tradutor de erros estável
 * (identidade só muda quando o idioma muda). */
export function useErrorMessage() {
  const { t } = useTranslation();
  return useCallback((err: unknown) => translateError(t, err), [t]);
}
