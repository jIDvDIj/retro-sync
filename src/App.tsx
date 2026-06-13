import { useState } from "react";

import { AddEmulator } from "./components/AddEmulator";
import { ConnectDrive } from "./components/ConnectDrive";
import { EmulatorCard } from "./components/EmulatorCard";
import { SyncStatus } from "./components/SyncStatus";
import { useEmulators } from "./hooks/useEmulators";
import { useSyncEvents } from "./hooks/useSyncEvents";
import "./App.css";

function App() {
  const sync = useSyncEvents();
  const { emulators, loading, error, refresh, remove } = useEmulators();
  const [connected, setConnected] = useState(false);

  return (
    <main className="app">
      <header className="app-header">
        <h1>RetroSync</h1>
        <ConnectDrive onConnectionChange={setConnected} />
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
    </main>
  );
}

export default App;
