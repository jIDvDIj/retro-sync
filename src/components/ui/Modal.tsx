import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import "./Modal.css";

interface Props {
  title: ReactNode;
  onClose: () => void;
  children: ReactNode;
}

/**
 * Shell padrão de modal: overlay escurecido + painel sólido elevado. Todos os
 * modais do app (configurações, conflito, pendências, histórico…) usam este
 * primitivo em vez de reimplementar o próprio overlay. Em viewport mobile o
 * painel ocupa a tela inteira (full-screen sheet).
 */
export function Modal({ title, onClose, children }: Props) {
  const { t } = useTranslation();
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h2>{title}</h2>
          <button className="secondary" onClick={onClose}>
            {t("common.close")}
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}
