import type { Localized } from "../types";
import type { sync as SyncEn } from "../en/sync";

export const sync: Localized<typeof SyncEn> = {
  sync: {
    syncNow: "Sincronizar agora",
    syncing: "Sincronizando…",
    justNow: "agora mesmo",
    secondsAgo: "há {{count}}s",
    minutesAgo: "há {{count}} min",
    hoursAgo: "há {{count}} h",
    queued: "pendentes {{count}}",
    failed: "falhas {{count}}",
    lastSync: "Último sync {{when}}",
    never: "Nenhuma sincronização ainda",
    backupBanner_one:
      "{{count}} arquivo local foi salvo em backup antes do primeiro sync (o Drive venceu).",
    backupBanner_other:
      "{{count}} arquivos locais foram salvos em backup antes do primeiro sync (o Drive venceu).",
    openBackupFolder: "Abrir pasta de backup",
    lastSyncError: "Falha no último sync{{emulator}}: {{message}}",
  },
  emulator: {
    conflictBadge: "conflito",
    running: "em execução",
    idle: "parado",
    resolveConflict_one: "Resolver conflito",
    resolveConflict_other: "Resolver conflito ({{count}})",
    removing: "Removendo…",
    remove: "Remover",
    games_one: "▸ {{count}} jogo",
    games_other: "▸ {{count}} jogos",
    hideGames: "▾ Ocultar jogos",
    noGames: "nenhum jogo sincronizado ainda",
  },
  conflict: {
    title: "Conflito — {{emulator}}",
    intro:
      "Estes arquivos mudaram neste dispositivo e no Drive desde o último sync. Escolha qual versão manter — o sync deste emulador está pausado até a resolução. A versão descartada localmente é salva em backup.",
    thisDevice: "Este dispositivo",
    drive: "Drive",
    keepLocal: "Manter local",
    keepDrive: "Manter do Drive",
  },
};
