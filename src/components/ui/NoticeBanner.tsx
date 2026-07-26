import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { dismissNotice, listDismissedNotices } from "../../lib/ipc";

import "./NoticeBanner.css";

interface Props {
  /** Identificador persistente: uma vez dispensado, o banner não reaparece. */
  id: string;
  tone?: "info" | "warning" | "success" | "danger";
  children: ReactNode;
}

/**
 * Banner informativo descartável:
 * cada banner tem um ID; ao fechar, o ID é persistido no backend e o banner
 * não volta a ser exibido — nem após reiniciar o app.
 */
export function NoticeBanner({ id, tone = "info", children }: Props) {
  const { t } = useTranslation();
  // `null` = ainda não sabemos se foi dispensado; não renderiza (evita flash).
  const [visible, setVisible] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    listDismissedNotices()
      .then((ids) => {
        if (!cancelled) setVisible(!ids.includes(id));
      })
      .catch(() => {
        if (!cancelled) setVisible(true);
      });
    return () => {
      cancelled = true;
    };
  }, [id]);

  if (!visible) return null;

  const dismiss = () => {
    setVisible(false);
    // Persistência best-effort: falha só faz o banner voltar na próxima sessão.
    dismissNotice(id).catch(() => {});
  };

  return (
    <div className={`notice-banner notice-${tone}`}>
      <div className="notice-content">{children}</div>
      <button
        className="notice-dismiss"
        onClick={dismiss}
        aria-label={t("common.dismiss")}
        title={t("common.dismiss")}
      >
        ×
      </button>
    </div>
  );
}
