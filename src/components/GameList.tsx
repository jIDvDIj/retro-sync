import { useTranslation } from "react-i18next";

import type { SyncedGame } from "../types/ipc";

/** Chaves i18n dos rótulos de categoria (reaproveitadas das configurações). */
const CATEGORY_LABEL = {
  saves: "settings.categories.saves",
  savestates: "settings.categories.savestates",
  config: "settings.categories.config",
} as const;

/** Tamanho legível a partir de bytes (B / KB / MB). */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${Math.round(kb)} KB`;
  return `${(kb / 1024).toFixed(1)} MB`;
}

/**
 * Lista de jogos sincronizados de um emulador: nome legível (ou serial), as
 * categorias em que tem arquivos e o tamanho total (FEATURE-001).
 */
export function GameList({ games }: { games: SyncedGame[] }) {
  const { t } = useTranslation();

  if (games.length === 0) {
    return <p className="muted empty game-empty">{t("emulator.noGames")}</p>;
  }

  return (
    <ul className="game-list">
      {games.map((game) => (
        <li key={`${game.emulator}/${game.serial}`} className="game-row">
          <span className="game-name" title={game.serial}>
            {game.name ?? game.serial}
          </span>
          <span className="game-cats">
            {game.categories.map((category) => (
              <span key={category} className="badge badge-cat">
                {t(CATEGORY_LABEL[category])}
              </span>
            ))}
          </span>
          <span className="game-size muted">{formatSize(game.sizeBytes)}</span>
        </li>
      ))}
    </ul>
  );
}
