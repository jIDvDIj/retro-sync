# Decisões Técnicas

Registro consolidado das decisões de design e seus trade-offs. Formato leve de ADR
(Architecture Decision Record). Cada decisão lista o **contexto**, a **escolha** e a
**justificativa/alternativas**.

---

## Frontend "burro", backend "inteligente"

**Contexto**: app Tauri tem duas linguagens; onde colocar a lógica?

**Escolha**: 100% da lógica de negócio no Rust. O React só dispara comandos e renderiza
estado recebido por eventos.

**Justificativa**: evita estado duplicado entre JS e Rust; mantém credenciais e tokens
fora do contexto JS (superfície de ataque menor); torna o frontend trivialmente
substituível. Custo: todo dado de UI precisa cruzar a boundary explicitamente.

---

## Escopo OAuth `drive.file`

**Contexto**: escopos do Drive vão de `drive.file` (só o que o app cria) a `drive` (tudo).

**Escolha**: `drive.file` + `openid email`.

**Justificativa**: é exatamente o que o RetroSync precisa (ele cria a pasta `RetroSync/`);
é **não-sensível**, o que evita o processo de verificação restrita do Google (auditoria
cara e lenta); reduz o risco para o usuário (o app não vê o resto do Drive dele).
Alternativa `drive` rejeitada por excesso de permissão e fricção de publicação.

---

## Proxy Worker esconde o `client_secret`

**Contexto**: o token endpoint do Google exige `client_secret`. Compilado no binário
(`option_env!`), ele é extraível de uma release (`strings`/descompilador) e pode ser usado
para abusar das credenciais do app conforme a base de usuários cresce.

**Escolha**: um Cloudflare Worker minúsculo intermedia `/token` e `/refresh`, guardando o
`client_secret` como secret cifrado do Cloudflare. O app só conhece a URL pública do Worker
e um `PROXY_SECRET` compartilhado (header `X-Proxy-Secret`). No CI, apenas `CLIENT_ID`,
`TOKEN_PROXY_URL` e `PROXY_SECRET` são injetados — o `client_secret` nunca entra no GitHub.

**Justificativa**: o `client_secret` deixa de existir em qualquer artefato distribuído ou
versionado. O redirect continua sendo o loopback `http://127.0.0.1:<porta>` tratado pelo app
— o Worker **não** é redirect URI, então o cliente OAuth permanece do tipo **Desktop app**
(único que aceita loopback em porta arbitrária).

**Trade-off aceito**: o `PROXY_SECRET` ainda é embutido no binário, logo extraível — barra
abuso casual e permite rotação, mas não é segredo forte. A proteção real é o `client_secret`
fora do binário. Suficiente para o porte do projeto; atestação de cliente fica fora de escopo.

Detalhes em [15 — Proxy Cloudflare Worker (OAuth)](./15-proxy-worker-oauth.md).

---

## OAuth2 com PKCE + redirect loopback

**Contexto**: app desktop nativo não tem como guardar um client secret de verdade.

**Escolha**: PKCE (RFC 7636) com redirect para `127.0.0.1:porta-efêmera` (RFC 8252). O
client secret exigido pelo Google para clientes Desktop vem de env, nunca do código, e a
segurança real vem do PKCE.

**Justificativa**: padrão da indústria para apps instalados (rclone, gcloud SDK). O
`code_verifier` nunca trafega na URL de autorização; só o `challenge` S256. `state`
aleatório protege contra CSRF.

---

## Token storage: keyring + memória

**Contexto**: onde guardar refresh e access tokens.

**Escolha**: refresh token no keychain nativo do SO (`keyring`); access token só em
memória, renovado automaticamente com margem de 60s.

**Justificativa**: keychain é o local seguro do SO para segredos. Access token é efêmero
e não precisa persistir. **Tokens nunca cruzam a boundary** — o frontend só vê
`AuthStatus`. A trait de storage permite fallback futuro no Linux (Secret Service ausente
em setups minimalistas).

---

## Manifest: SQLite + snapshot JSON

**Contexto**: a spec pedia `sync_manifest.json` no Drive. JSON é frágil para estado
operacional (concorrência, consultas, corrupção).

**Escolha**: a **fonte de verdade operacional** é a tabela SQLite local
(`sync_manifest`); o `sync_manifest.json` no Drive é um **snapshot exportado** a cada sync.

**Justificativa**: SQLite é transacional, consultável e resistente a corrupção; serve à
fila offline e ao diff. O snapshot JSON cumpre a estrutura especificada e serve para
diagnóstico e bootstrap rápido de uma segunda máquina. Custo: duas representações, mas o
JSON é derivado (write-only do ponto de vista do app).

---

## Resolução de conflito por timestamp

**Contexto**: sync bidirecional precisa decidir quem vence quando um arquivo difere.

**Escolha**: o mais recente vence, com **tolerância de ±2s** e o **par de mtimes do último
sync** registrado no manifest. Nunca deleta.

**Justificativa**:
- A tolerância absorve granularidade de filesystem e pequenos desvios de relógio.
- O par `(local, drive)` do último sync distingue "nada mudou" de "mudou de um lado" —
  essencial porque os relógios local e remoto divergem; sem isso, qualquer skew causaria
  re-sync eterno.
- Uploads gravam o mtime local em `modifiedTime`; downloads aplicam o `modifiedTime` do
  Drive no arquivo local. Os dois lados convergem para o mesmo timestamp.
- Como a v1.0 nunca deleta, o pior caso é um save antigo sobrescrito no lado perdedor — e
  o histórico de revisões do Drive ainda permite resgate manual.

Alternativa (hash de conteúdo) rejeitada para a v1.0 por custo de I/O e porque timestamp
resolve o caso comum (um save por vez, numa máquina por vez).

---

## Fila offline como registro de intenção

**Contexto**: como retomar transferências que falharam por rede/arquivo em uso.

**Escolha**: a pendência registra *que* um arquivo precisa sincronizar, não *como*. O
próximo sync re-detecta a diferença pelo diff (fonte da verdade) e refaz a operação;
`resolve` limpa a pendência ao concluir.

**Justificativa**: imune a replay de operação obsoleta (ex.: enfileirou um upload, mas o
arquivo mudou de novo antes do retry). Mais simples que uma fila de comandos com payload.
A tabela tem dedupe (`UNIQUE`) e contagem de tentativas para diagnóstico.

---

## Engine agnóstico a emuladores (`SyncTarget`)

**Contexto**: a arquitetura precisa suportar emuladores novos sem reescrever o sync.

**Escolha**: o engine opera sobre `SyncTarget` (rótulo + listas de caminhos). A conversão
`EmulatorProfile → SyncTarget` é função de dados, fora do engine.

**Justificativa**: adicionar RetroArch/Dolphin é só um novo arquivo em `emulator/`.
`sync/` nunca muda. Testável isoladamente (o diff e o conflito não tocam disco real além
do scan).

---

## Process watcher: abertura imediata, fechamento com debounce

**Contexto**: o watcher de `sysinfo` ocasionalmente não lista um processo num tick, e
emuladores spawnam processos auxiliares — ambos causam flapping. Mas os dois gatilhos têm
urgências opostas.

**Escolha**: `EmulatorStarted` é emitido **no primeiro tick** em que o processo aparece;
`EmulatorStopped` só após `WATCHER_STOP_DEBOUNCE_TICKS` (2) ticks consecutivos ausente
(≈4s). A máquina de estados (`RunStateTracker::reconcile`) é pura, sem `sysinfo`.

**Justificativa**: baixar os saves do Drive (abertura → Drive → Local) deve acontecer o
quanto antes, antes de o jogo ler os arquivos — atraso aqui é prejudicial. Já declarar
"fechou" cedo demais dispararia um upload Local → Drive no meio de um flicker, então vale
esperar a confirmação. Separar a lógica pura do `sysinfo` torna o debounce 100% testável.
Alternativa (debounce simétrico) rejeitada por atrasar o download de abertura sem ganho.

---

## Watcher: `sysinfo` síncrono em `spawn_blocking`, estado dinâmico via SQLite

**Contexto**: `refresh_processes` é bloqueante; a spec pede loop `tokio::time::interval` +
`mpsc`. A lista de emuladores a monitorar muda em runtime (`add_emulator`/`remove_emulator`).

**Escolha**: o `System` e o `RunStateTracker` viajam para dentro de um `spawn_blocking` a
cada tick e voltam com os eventos; o produtor relê os emuladores do SQLite a cada tick.
Refresh com `ProcessRefreshKind::nothing()` (só o nome do processo).

**Justificativa**: mover o estado para o thread bloqueante mantém o runtime async livre sem
recriar o `System` (caro) a cada poll. Reler o SQLite a cada 2s é barato (local, poucos
registros) e dispensa um canal extra de invalidação. O refresh mínimo mantém o tick leve.

---

## `rustls` em vez de OpenSSL

**Contexto**: o TLS padrão do reqwest exige OpenSSL do sistema.

**Escolha**: `reqwest` com `rustls-tls` e `default-features = false`.

**Justificativa**: TLS puro Rust, mesma stack em Windows/Linux/macOS, sem dependência de
biblioteca de sistema — melhor para distribuição, não só para o dev no WSL.

---

## Retry centralizado no transporte

**Contexto**: a regra exige retry exponencial (máx 3) em cada chamada ao Drive.

**Escolha**: um único `send_with_retry` em `drive/client.rs` por onde passa toda chamada;
a closure `build` reconstrói o request a cada tentativa.

**Justificativa**: evita espalhar lógica de retry por dezenas de chamadas. Trata 401
(renova token), 429/403-rate-limit/5xx e falha de rede de forma uniforme. Backoff
500ms/1s/2s + jitter. Concorrência limitada a 3 transferências por semáforo lógico
(`buffer_unordered`).

---

## App vive na tray; fechar a janela ≠ sair

**Contexto**: o gatilho "sync ao fechar o RetroSync" precisa rodar de forma confiável.

**Escolha**: fechar a janela (`WindowEvent::CloseRequested`) apenas a esconde
(`prevent_close` + `hide`); o app continua na bandeja. O sync de despedida
(`TRIGGER_SHUTDOWN`) roda no handler do menu **"Sair"**, imediatamente antes de
`app.exit(0)`.

**Justificativa**: como fechar a janela só minimiza para a tray, o único caminho de saída
real passa pelo "Sair" — então o sync de despedida sempre executa. Coloquei o sync nesse
handler em vez do `RunEvent::ExitRequested` (a ideia inicial) porque é uma saída
intencional e controlável: evita a dança de `prevent_exit` + re-disparar o exit depois do
sync async. Todas as operações de tray/janela são feitas no Rust, então não exigem
permissões novas nas capabilities.

---

## Último sync em célula compartilhada

**Contexto**: a UI mostra o "último sync", mas o startup sync roda antes de a tela montar
e o estado do React se perde se o app reiniciar.

**Escolha**: `Arc<Mutex<Option<LastSync>>>` compartilhado entre o `SyncEngine` (escreve ao
concluir, **antes** de emitir `sync:completed`) e o `AppState` (lê via `get_last_sync`). A
UI busca no mount e atualiza ao vivo pelo evento.

**Justificativa**: cobre o mount tardio sem persistir nada (o estado é efêmero por
execução, e cada inicialização gera um sync novo de qualquer forma). Gravar antes de emitir
o evento garante que o `get_last_sync` disparado no `sync:completed` seja consistente.
Alternativa (nova tabela SQLite + migração v2) rejeitada por excesso para um dado volátil.

---

## Notificações de erro no backend

**Contexto**: a spec cita o plugin JS `@tauri-apps/plugin-notification` para erros críticos
de sync.

**Escolha**: disparar a notificação no **backend** (`NotificationExt` do Rust), no mesmo
ponto em que o engine emite `sync:error`.

**Justificativa**: o sync acontece no backend e precisa notificar mesmo quando a janela
está oculta (gatilhos de startup/watcher) ou durante o sync de despedida no shutdown,
quando o webview pode já não estar responsivo. Disparar do frontend dependeria do webview
vivo. O plugin continua inicializado no JS, então a alternativa pelo frontend segue
disponível se necessário.

---

## Tipos compartilhados espelhados manualmente

**Contexto**: drift entre struct Rust e interface TS quebra em runtime, não em compile time.

**Escolha**: espelhamento manual concentrado em `src/types/ipc.ts` (TS) e nas structs com
`#[serde(rename_all = "camelCase")]` (Rust), com testes de serialização. Migrar para
`ts-rs` se o número de tipos crescer.

**Justificativa**: para a quantidade atual de tipos, manual + testes é suficiente e sem
dependência extra. O drift do `file_busy` (encontrado ao documentar) mostra o risco — daí
os testes de serialização e a centralização num arquivo só de cada lado.
