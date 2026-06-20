/** Inglês — idioma padrão. A forma deste objeto é a fonte da verdade (`Resources`). */
export const en = {
  common: {
    close: "Close",
    add: "Add",
  },
  app: {
    checkingConnection: "Checking Google Drive connection…",
    settings: "⚙ Settings",
    emulators: "Emulators",
    loading: "loading…",
    noEmulators:
      "No emulators configured yet. Use “Add emulator” and select the PPSSPP or PCSX2 root folder.",
  },
  login: {
    tagline: "Sync your emulators’ saves, savestates and configs with Google Drive.",
    permissionNote:
      "RetroSync <strong>does not access your personal data</strong>. It can only see and modify the files it creates in your Google Drive.",
    connecting: "Waiting for authorization in the browser…",
    connect: "Connect to Google Drive",
  },
  device: {
    nameLabel: "This device’s name",
    namePlaceholder: "e.g. Gaming PC, Laptop",
  },
  account: {
    connected: "Google account connected",
    disconnect: "Disconnect",
  },
  sync: {
    syncNow: "Sync now",
    syncing: "Syncing…",
    justNow: "just now",
    secondsAgo: "{{count}}s ago",
    minutesAgo: "{{count}} min ago",
    hoursAgo: "{{count}} h ago",
    queued: "queued {{count}}",
    failed: "failed {{count}}",
    lastSync: "Last sync {{when}}",
    never: "No sync yet",
    backupBanner_one: "{{count}} local file was backed up before the first sync (Drive won).",
    backupBanner_other: "{{count}} local files were backed up before the first sync (Drive won).",
    openBackupFolder: "Open backup folder",
    lastSyncError: "Last sync failed{{emulator}}: {{message}}",
  },
  emulator: {
    conflictBadge: "conflict",
    running: "running",
    idle: "stopped",
    resolveConflict_one: "Resolve conflict",
    resolveConflict_other: "Resolve conflict ({{count}})",
    removing: "Removing…",
    remove: "Remove",
  },
  conflict: {
    title: "Conflict — {{emulator}}",
    intro:
      "These files changed on this device and on Drive since the last sync. Choose which version to keep — syncing this emulator is paused until you resolve it. The version discarded locally is saved to backup.",
    thisDevice: "This device",
    drive: "Drive",
    keepLocal: "Keep local",
    keepDrive: "Keep Drive’s",
  },
  settings: {
    title: "Settings",
    device: {
      heading: "Device",
      hint: "Identifies this machine in the sync metadata. Changing it here doesn’t require signing in again.",
      save: "Save name",
      saving: "Saving…",
      saved: "Saved ✓",
    },
    language: {
      heading: "Language",
      hint: "Changes the app interface language.",
      label: "Interface language",
    },
    autoSync: {
      heading: "Automatic sync",
      hint: "Even with everything off, the “Sync now” button stays available.",
    },
    startup: {
      heading: "Startup",
      hint: "Launches RetroSync with the system, straight to the tray, to sync in the background without you opening the app.",
      label: "Launch on system startup",
      sublabel: "runs in the background when the computer starts",
    },
    notif: {
      heading: "Notifications",
      hint: "Frequent automatic syncs can produce intrusive notifications — reduce the noise here.",
      label: "Native notification level",
      all: "Everything (sync, errors, emulator detected)",
      errorsOnly: "Errors only",
      none: "None",
    },
    categories: {
      heading: "Per-emulator sync",
      hint: "Choose which categories to sync. Turn off “Config” to avoid sharing resolution and controls across different devices.",
      empty: "Add an emulator to configure its categories.",
      saves: "Saves",
      savestates: "Savestates",
      config: "Config",
    },
    triggers: {
      startupLabel: "When RetroSync opens",
      startupHint: "syncs when the app starts",
      emulatorStartLabel: "Before opening the emulator",
      emulatorStartHint: "downloads fresh saves from Drive",
      emulatorStopLabel: "When closing the emulator",
      emulatorStopHint: "uploads the session’s saves",
    },
    backups: {
      heading: "Backups",
      hint: "Copies RetroSync keeps before overwriting a local file — on the first sync of a file that already exists on Drive, or when resolving a conflict by keeping Drive’s version. Nothing is deleted.",
      open: "Open backups folder",
    },
  },
  addEmulator: {
    button: "Add emulator",
    title: "Add emulator",
    recommended: "Recommended",
    searching: "looking for installed emulators…",
    noneDetected: "No new emulators detected automatically.",
    installedNoSaves: "installed, no saves yet",
    adding: "Adding…",
    openOnce: "open the emulator once",
    pickFolder: "Point to a folder",
    pickFolderHint: "For portable installs or emulators not in the list, select the root folder.",
    detecting: "Detecting…",
    selectFolder: "Select folder…",
    detectedHere: "detected in this folder",
    manualIntro:
      "No emulator recognized in this folder. Enter the details manually — the folders must be inside the root.",
    addManual: "Add manually",
    selectSubfolder: "Select subfolder…",
    subfolderError: "select a subfolder inside the root folder",
    nameLabel: "Name",
    namePlaceholder: "e.g. Dolphin",
    sourceSavesFound: "saves found",
    sourceInstalled: "installed",
    pickRootTitle: "Select the emulator’s root folder",
    pickSubTitle: "Select a subfolder of the root",
  },
  errors: {
    io: "I/O error",
    database: "Database error",
    network: "Network error",
    keyring: "Credentials vault error",
    serialization: "Serialization error",
    auth: "Authentication error",
    emulator_not_detected: "Emulator not recognized in folder",
    emulator_exists: "An emulator with this name already exists",
    file_busy: "File in use (modified while reading)",
    unexpected: "Unexpected error talking to the backend",
  },
} as const;

export type Resources = typeof en;

/** Mesma forma de `Resources`, mas com folhas `string` — usada para tipar os
 * demais idiomas e garantir que tenham exatamente as mesmas chaves. */
export type Localized<T> = {
  [K in keyof T]: T[K] extends string ? string : Localized<T[K]>;
};
