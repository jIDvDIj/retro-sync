# 06 — Monitoramento de Processos

**Commit**: `60a0ae6` — *feat: process watcher dispara sync ao abrir/fechar emuladores*

Detecta quando um emulador configurado abre ou fecha e dispara o sync direcionado:
abrir → Drive → Local (saves frescos antes do jogo carregar); fechar → Local → Drive
(sobe os saves da sessão). São dois dos cinco gatilhos de sync da especificação.

## Arquivos

| Arquivo | Conteúdo |
| --- | --- |
| `watcher/mod.rs` | Canal `mpsc`, tasks produtor/consumidor, `WatcherEvent`, `EmulatorStatusEvent` |
| `watcher/process_watcher.rs` | `RunStateTracker` (debounce, pura) + `poll_once` (`sysinfo`) |
| `emulator/mod.rs` | `process_names(name)` — nomes de processo por emulador |
| `emulator/ppsspp.rs`, `pcsx2.rs` | `PROCESS_NAMES` (passam a ser usados) |
| `sync/engine.rs` | `sync_emulator` (já existia; deixa de ser dead code) |
| `constants.rs` | `WATCHER_STOP_DEBOUNCE_TICKS` |
| `lib.rs` | `watcher::start(db, engine, app)` no `setup` |
| `src/types/ipc.ts` | `EmulatorStatusEvent` |

## Arquitetura: produtor → `mpsc` → consumidor

Duas tasks assíncronas ligadas por um canal `tokio::sync::mpsc`, exatamente como a
especificação pede:

- **Produtor** (`spawn_poll_loop`): loop `tokio::time::interval` de `WATCHER_POLL_INTERVAL_SECS`
  (2s). A cada tick: lê os emuladores configurados do SQLite, atualiza a lista de processos
  via `sysinfo` e publica as transições no canal.
- **Consumidor** (`spawn_consumer`): para cada `WatcherEvent`, emite `emulator:status` ao
  frontend e chama `engine.sync_emulator(name, direção, trigger)`.

Desacoplar os dois mantém o polling leve e nunca bloqueado por um sync em andamento (o
engine já serializa execuções com seu próprio `Mutex`).

## `sysinfo` dentro de `spawn_blocking`

`refresh_processes` é síncrono. O `System` e o `RunStateTracker` persistem entre ticks e
**viajam para dentro do `spawn_blocking` a cada poll**, voltando com os eventos — assim o
runtime async nunca trava. O refresh usa `refresh_processes_specifics(..., ProcessRefreshKind::nothing())`:
o nome do processo vem de graça, sem coletar memória/CPU/disco, mantendo o tick de 2s barato.

O matching é por **igualdade exata case-insensitive** entre o nome do processo e os
`PROCESS_NAMES` do perfil — igualdade (não `contains`) evita falso positivo. Os nomes são
recolhidos num `HashSet` uma vez por tick: custo O(processos) + O(monitorados).

## Debounce: abertura imediata, fechamento com atraso

A máquina de estados vive em `RunStateTracker::reconcile`, **pura e sem `sysinfo`** —
recebe "quais emuladores estão presentes neste tick" e devolve as transições. Por isso é
100% testável sem o SO.

- **Abertura** emite `EmulatorStarted` **imediatamente**: baixar os saves do Drive deve
  acontecer o quanto antes, antes do jogo ler os arquivos.
- **Fechamento** só emite `EmulatorStopped` após `WATCHER_STOP_DEBOUNCE_TICKS` (2) ticks
  consecutivos sem o processo (≈4s). Protege contra flapping do `sysinfo` ou processos
  auxiliares que o emulador spawna.
- Um emulador **removido** da configuração é esquecido em silêncio (sem `Stopped`) — não
  queremos disparar sync ao desconfigurar.

Ver [Decisões técnicas](./decisoes-tecnicas.md#process-watcher-abertura-imediata-fechamento-com-debounce).

## Lista de monitorados dinâmica

O produtor consulta o SQLite (`emulators::list`) **a cada tick**, então `add_emulator` e
`remove_emulator` passam a valer sem reiniciar nada. `emulator::process_names(name)` mapeia
o nome canônico do perfil para os nomes de processo do SO; perfis sem nomes conhecidos são
ignorados.

## Direção do sync por gatilho

| Transição | Direção | Trigger |
| --- | --- | --- |
| `EmulatorStarted` | `DriveToLocal` | `emulator-start` |
| `EmulatorStopped` | `LocalToDrive` | `emulator-stop` |

## Evento ao frontend

`emulator:status` (`EVT_EMULATOR_STATUS`) com payload `EmulatorStatusEvent { emulator, running }`
— já prepara a UI do Passo 7 para mostrar quando um emulador está em execução. Ver
[Referência — Boundary IPC](./referencia-ipc.md#eventos).

## Testes (8 novos, 52 no total)

Todos sobre `RunStateTracker::reconcile` (a parte sujeita a bug; o caminho do `sysinfo` é
fino e não testável sem os processos reais):

- abertura emite `Started` imediatamente; presença contínua não reemite;
- encerramento só após o debounce; flap curto não emite `Stopped`;
- emulador nunca presente não emite nada; ciclo completo start→stop→start;
- remoção do monitoramento não emite `Stopped` e esquece o estado;
- dois emuladores rastreados de forma independente.

## Como testar

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # 52 testes (8 do watcher)
```

Manual no Windows (`npm run tauri dev`, com o Drive conectado e um emulador via
`add_emulator`):

1. Abra o PPSSPP/PCSX2 → o log mostra `transição de emulador detectada (running=true)` e
   dispara um sync Drive → Local;
2. Jogue, salve e feche o emulador → após ≈4s, `running=false` e um sync Local → Drive
   sobe os saves da sessão;
3. Acompanhe em `%LOCALAPPDATA%\com.retrosync.app\logs\retrosync.log`.

> O caminho real do `sysinfo` não é exercitável no WSL (sem GUI nem emuladores), mas toda
> a lógica de decisão — a parte sujeita a bug — está coberta por testes.
