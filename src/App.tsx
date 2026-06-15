import { useState } from "react";

import { AccountStatus } from "./components/AccountStatus";
import { AddEmulator } from "./components/AddEmulator";
import { EmulatorCard } from "./components/EmulatorCard";
import { LoginScreen } from "./components/LoginScreen";
import { SettingsModal } from "./components/SettingsModal";
import { SyncStatus } from "./components/SyncStatus";
import { useAuth } from "./hooks/useAuth";
import { useConflicts } from "./hooks/useConflicts";
import { useEmulators } from "./hooks/useEmulators";
import { useSettings } from "./hooks/useSettings";
import { useSyncEvents } from "./hooks/useSyncEvents";
import "./App.css";

function App() {
  const auth = useAuth();
  const { settings, reload: reloadSettings } = useSettings();

  // Enquanto o status de auth não chega, não decidimos qual tela mostrar.
  if (auth.loading) {
    return (
      <main className="login-screen">
        <p className="muted">verificando conexão com o Google Drive…</p>
      </main>
    );
  }

  // Sem login, a única tela acessível é a de login.
  if (!auth.connected) {
    return (
      <LoginScreen
        initialDeviceName={settings?.deviceName ?? null}
        onConnected={(status) => {
          auth.setStatus(status);
          reloadSettings();
        }}
      />
    );
  }

  return <MainScreen auth={auth} settings={settings} reloadSettings={reloadSettings} />;
}

interface MainScreenProps {
  auth: ReturnType<typeof useAuth>;
  settings: ReturnType<typeof useSettings>["settings"];
  reloadSettings: () => void;
}

/**
 * Tela principal — só montada quando o usuário está conectado. Os hooks de
 * emuladores/sync/conflitos vivem aqui para não rodar na tela de login.
 */
function MainScreen({ auth, settings, reloadSettings }: MainScreenProps) {
  const sync = useSyncEvents();
  const { emulators, loading, error, refresh, remove } = useEmulators();
  const { conflicts, reload: reloadConflicts } = useConflicts();
  const [showSettings, setShowSettings] = useState(false);

  return (
    <main className="app">
      <header className="app-header">
        <h1>RetroSync</h1>
        <div className="header-actions">
          <AccountStatus
            email={auth.status?.email ?? null}
            deviceName={settings?.deviceName ?? null}
            onDisconnect={auth.disconnect}
            error={auth.error}
          />
          <button className="secondary" onClick={() => setShowSettings(true)}>
            ⚙ Configurações
          </button>
        </div>
      </header>

      <section className="emulators">
        <div className="section-head">
          <h2>Emuladores</h2>
          <AddEmulator onAdded={refresh} />
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
                conflicts={conflicts.filter((c) => c.emulator === profile.name)}
                onRemove={remove}
                onConflictResolved={reloadConflicts}
              />
            ))}
          </div>
        )}
      </section>

      <SyncStatus state={sync} />

      {showSettings && settings ? (
        <SettingsModal
          settings={settings}
          emulators={emulators}
          onClose={() => setShowSettings(false)}
          onSaved={reloadSettings}
        />
      ) : null}
    </main>
  );
}

export default App;
