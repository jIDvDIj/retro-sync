# Análise: Adoção de `tauri-specta` no RetroSync

## Contexto

A boundary IPC do RetroSync exige sincronização manual de **três lugares** a cada mudança de struct, enum ou evento:

1. **Rust** — a struct com `#[derive(Serialize, Deserialize)]` e `#[serde(rename_all = "camelCase")]`
2. **`src/types/ipc.ts`** — a interface TypeScript espelho (mantida à mão)
3. **`src/lib/ipc.ts`** — o wrapper tipado de `invoke()` (mantido à mão)

Não há nenhuma verificação em tempo de compilação. Uma divergência produz `undefined` silencioso em runtime — detectável apenas por testes manuais ou crashes na UI.

### Escopo atual da boundary

| Categoria | Itens |
|---|---|
| Structs cruzando a boundary | `HealthStatus`, `AuthStatus`, `Settings`, `TriggerSettings`, `EmulatorProfile`, `DiscoveredEmulator`, `SyncCategories`, `SyncProgress`, `SyncSummary`, `Conflict`, `SyncStarted`, `LastSync`, `SyncErrorEvent`, `EmulatorStatusEvent`, `AppErrorPayload` |
| Enums | `NotificationLevel`, `DiscoverySource`, `SyncDirection`, `ConflictResolution` + union `code` de `AppErrorPayload` |
| Eventos | 7 constantes (`sync:started`, `sync:progress`, `sync:completed`, `sync:error`, `sync:conflict`, `auth:status`, `emulator:status`) |
| Comandos invoke | 18 funções em `src/lib/ipc.ts` |

Ao todo, **~15 tipos + 7 eventos + 18 comandos** dependem dessa sincronia.

---

## O que é `tauri-specta`

`tauri-specta` é um plugin para Tauri que usa a crate `specta` para inspecionar os tipos Rust em tempo de compilação e gerar um arquivo `.ts` com as interfaces correspondentes. Em vez de manter `src/types/ipc.ts` à mão, ele é gerado automaticamente ao rodar `cargo build` (ou um binário auxiliar).

Funciona em duas partes:

- **`specta`** — derive macro `#[derive(Type)]` que extrai o schema do tipo
- **`tauri-specta`** — integra com o `Builder` do Tauri para exportar comandos e eventos tipados

---

## Ganhos

### 1. Elimina a sincronização manual de `src/types/ipc.ts`

O arquivo deixa de ser mantido à mão e passa a ser gerado. Ao adicionar um campo em `SyncSummary` no Rust, o TypeScript é atualizado automaticamente na próxima build — sem risco de esquecimento.

### 2. Drift detectado em tempo de build, não em runtime

Com a geração automática, qualquer divergência é impossível por construção: o arquivo TS é sempre derivado do Rust. Hoje uma struct desatualizada produz `undefined` silencioso; com specta, não existe essa janela.

### 3. Menor custo para adicionar novos comandos e tipos

Para cada novo comando hoje: atualizar Rust + `ipc.ts` + `ipc.ts` wrappers. Com specta: atualizar Rust + `ipc.ts` wrappers (tipos vêm grátis). Para um evento novo: Rust + constants TS (EVT) — o payload type vem gerado.

### 4. Eventos também podem ser gerados

`tauri-specta` suporta geração de eventos tipados. O objeto `EVT` e os tipos de payload de cada evento (`SyncProgress`, `EmulatorStatusEvent`, etc.) podem ser gerados junto com os tipos de struct.

### 5. Onboarding mais seguro

Um novo contribuidor que altera uma struct Rust não precisa saber que `src/types/ipc.ts` existe. O build quebra imediatamente se ele esquecer de rodar a geração — ou melhor, o CI já gera e commita automaticamente.

---

## Perdas

### 1. Dependência nova com história mais curta que o Tauri

`tauri-specta` é um projeto da comunidade (não mantido pela Tauri Board). Tem boa adoção mas um histórico menor que o próprio Tauri. Risco de lag em atualizações de breaking changes do Tauri v2.

**Mitigação**: o projeto já usa plugins de terceiros (`tauri-plugin-autostart`, `tauri-plugin-single-instance`). O risco é real mas gerenciável.

### 2. Todas as structs da boundary precisam de `#[derive(specta::Type)]`

São ~15 structs e 4 enums espalhados por `commands.rs`, `auth/`, `sync/`, `storage/`, `emulator/` e `watcher/`. Cada uma precisa do derive adicional. É trabalho de migração, não de manutenção.

### 3. `src/types/ipc.ts` deixa de ser editável

O arquivo passa a ser gerado — qualquer edição manual é sobrescrita na próxima build. Isso exige uma mudança de hábito: anotações ou customizações no tipo TS precisam ir para o Rust (o que geralmente é a abordagem certa).

### 4. `src/lib/ipc.ts` continua manual

Os wrappers de `invoke()` não são gerados pelo specta. Ainda é necessário manter um arquivo à mão — só que um, não dois. A boundary se reduz de 3 arquivos para 2.

### 5. `AppErrorPayload.code` é um caso especial

O enum `AppError` no Rust mapeia para um union de string literal em `ipc.ts` (`"io" | "database" | ...`). Com specta, o enum Rust seria gerado como union TS automaticamente — mas o shape de serialização atual (`{ code, message, detail }`) é construído à mão em `error.rs` via `Serialize` customizado. Precisaria ser revisitado para ficar compatível com a geração.

### 6. Geração precisa rodar antes do `tsc`

O workflow de CI e de desenvolvimento precisa garantir que a geração acontece antes do build TypeScript. Isso adiciona um passo explícito (e.g., `cargo run --bin generate-bindings` ou integração no `build.rs`).

---

## Resumo comparativo

| Aspecto | Situação atual | Com tauri-specta |
|---|---|---|
| Sincronização de tipos | 3 arquivos manuais | 2 arquivos (1 gerado) |
| Drift Rust↔TS | Detectado em runtime | Impossível por construção |
| Custo por novo comando | Alto (3 arquivos) | Médio (1 arquivo + Rust) |
| Custo por novo campo | Médio (2 arquivos) | Baixo (só Rust) |
| Dependências extras | — | `specta`, `tauri-specta` |
| Complexidade de build | Baixa | Leve aumento (step extra) |
| Maturidade da solução | N/A | Boa, mas menor que Tauri |

---

## Esforço de migração estimado

1. Adicionar `specta` e `tauri-specta` ao `Cargo.toml` e ao `package.json` (ou usar a geração Rust pura sem npm).
2. Adicionar `#[derive(specta::Type)]` às ~19 structs/enums da boundary — 1 a 2 horas de trabalho mecânico.
3. Ajustar `AppError` para ser gerado corretamente (ou manter o union manual com um teste de regressão).
4. Criar o binário/script de geração e integrá-lo ao CI antes do step de `npm run build`.
5. Deletar `src/types/ipc.ts` (substituído pelo gerado) e ajustar o gitignore.

**Estimativa total**: 4–8 horas. Sem risco de regressão funcional — é puramente infraestrutura de tipos.

---

## Recomendação

**Vale a pena adotar**, com prioridade média-baixa.

O RetroSync ainda é pequeno (18 comandos, ~15 tipos) — o custo da manutenção manual é suportável hoje. Mas a boundary só cresce: cada novo emulador adiciona detecção, cada nova feature adiciona comandos. A adopção antecipada custa pouco e elimina uma fonte de bugs silenciosos que se torna mais cara à medida que o projeto cresce.

O momento ideal é o início de uma sprint de feature nova — assim a migração vem junto com as anotações dos novos tipos, amortizando o esforço.
