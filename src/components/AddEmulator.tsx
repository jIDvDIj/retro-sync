import { useState } from "react";
import { useTranslation } from "react-i18next";

import { AddEmulatorModal } from "./AddEmulatorModal";

interface Props {
  onAdded: () => void;
  /** Emuladores já configurados — repassados ao modal para filtrar sugestões. */
  existingNames: string[];
  disabled?: boolean;
}

/** Botão que abre o modal de adicionar emulador (recomendados + manual). */
export function AddEmulator({ onAdded, existingNames, disabled }: Props) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  return (
    <div className="add-emulator">
      <button onClick={() => setOpen(true)} disabled={disabled}>
        {t("addEmulator.button")}
      </button>
      {open ? (
        <AddEmulatorModal
          existingNames={existingNames}
          onClose={() => setOpen(false)}
          onAdded={onAdded}
        />
      ) : null}
    </div>
  );
}
