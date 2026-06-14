import { useState } from "react";

import { AddEmulator } from "./components/AddEmulator";
import { ConnectDrive } from "./components/ConnectDrive";
import { EmulatorCard } from "./components/EmulatorCard";
import { SettingsModal } from "./components/SettingsModal";
import { SyncStatus } from "./components/SyncStatus";
import { useEmulators } from "./hooks/useEmulators";
import { useSettings } from "./hooks/useSettings";
import { useSyncEvents } from "./hooks/useSyncEvents";
import "./App.css";

function App() {
  const sync = useSyncEvents();
  const { emulators, loading, error, refresh, remove } = useEmulators();
  const { settings, reload: reloadSettings } = useSettings();
  const [connected, setConnected] = useState(false);
  const [showSettings, setShowSettings] = useState(false);

  return (
    <main className="app">
      <header className="app-header">
        <h1>RetroSync</h1>
        <div className="header-actions">
          <ConnectDrive
            deviceName={settings?.deviceName ?? null}
            onConnectionChange={setConnected}
            onAfterConnect={reloadSettings}
          />
          {connected ? (
            <button className="secondary" onClick={() => setShowSettings(true)}>
              ⚙ Configurações
            </button>
          ) : null}
        </div>
      </header>

      <section className="emulators">
        <div className="section-head">
          <h2>Emuladores</h2>
          <AddEmulator onAdded={refresh} disabled={!connected} />
        </div>

        {loading ? (
          <p className="muted">carregando…</p>
        ) : error ? (
          <p className="error">{error}</p>
        ) : emulators.length === 0 ? (
          <p className="muted empty">
            Nenhum emulador configurado. Use “Adicionar emulador” e selecione a pasta raiz do PPSSPP
            ou PCSX2.
          </p>
        ) : (
          <div className="emulator-grid">
            {emulators.map((profile) => (
              <EmulatorCard
                key={profile.name}
                profile={profile}
                running={sync.running.has(profile.name)}
                onRemove={remove}
              />
            ))}
          </div>
        )}
      </section>

      <SyncStatus state={sync} connected={connected} />

      {showSettings && settings ? (
        <SettingsModal
          settings={settings}
          onClose={() => setShowSettings(false)}
          onSaved={reloadSettings}
        />
      ) : null}
    </main>
  );
}

export default App;
