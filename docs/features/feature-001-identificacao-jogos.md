# FEATURE-001 — Identificação de jogos sincronizados por emulador

**Status:** proposta  
**Componentes afetados:** `src-tauri/src/emulator/`, `src-tauri/src/storage/`, `src-tauri/src/commands.rs`, `src/`

---

## Objetivo

Exibir na UI, para cada emulador configurado, a lista de jogos cujos arquivos foram
sincronizados — com nome legível em vez de serial técnico.

---

## Viabilidade por emulador

### PPSSPP

Os saves ficam em `PSP/SAVEDATA/<SERIAL>/`, onde `<SERIAL>` é o identificador do jogo
(ex: `ULUS12345`). O `rel_path` já gravado no `sync_manifest` tem o serial como primeiro
componente — nenhuma mudança no backend de sync é necessária para extrair essa informação.

### PCSX2

| Categoria | Identificável? | Motivo |
|---|---|---|
| Memory cards (`memcards/`) | Não | Arquivo monolítico (`Mcd001.ps2`) com saves de todos os jogos misturados |
| Savestates (`sstates/`) | Sim | Nome de arquivo segue o padrão `<SERIAL>.<SLOT>.p2s` |

Para memory cards do PCSX2, não é possível identificar jogos individuais pelo caminho —
a granularidade mínima é o próprio arquivo de memory card.

---

## Resolução de serial → nome legível

O serial extraído do caminho (`ULUS12345`, `SLUS-12345`) não é legível pelo usuário.
Para exibir o nome do jogo são necessárias três opções:

### Opção A — OpenVGDB (recomendada)

Banco SQLite open source com mapeamento serial → nome para PSP, PS2 e outras plataformas.
Pode ser **empacotado junto com o binário Tauri** como asset, sem dependência de rede.
Funciona offline. A desvantagem é que precisa de atualização manual do arquivo quando
novos títulos são adicionados (raro para plataformas retroativas).

**Prós:** offline, sem autenticação, tamanho de arquivo pequeno, adequado para empacotar.  
**Contras:** cobertura pode ter lacunas; requer redistribuição do asset com novas versões do app.

### Opção B — ScreenScraper API

API focada em emulação, usada por RetroArch e Skraper. Suporta busca por serial de PSP e
PS2, retornando nome, plataforma e metadados. Requer cadastro gratuito para obter
credenciais.

**Prós:** cobertura ampla, dados sempre atualizados, suporta capas de jogos (extensível).  
**Contras:** requer conexão de rede; adiciona dependência externa; precisa de credenciais
armazenadas; rate limiting nas contas gratuitas.

### Opção C — IGDB (Twitch)

Base de dados ampla com API REST. Não é focada em emulação — a busca por serial é indireta
(por nome + plataforma). Requer autenticação OAuth via conta Twitch.

**Prós:** base bem mantida, cobertura abrangente.  
**Contras:** autenticação complexa; busca por serial não nativa; overhead desnecessário para
o caso de uso.

**Opção recomendada:** OpenVGDB para v1.0 — sem dependências de rede, sem autenticação,
empacotável. ScreenScraper pode ser avaliada se capas de jogos forem desejadas no futuro.

---

## O que seria necessário implementar

### Backend (Rust)

| Arquivo | Mudança |
|---|---|
| `src-tauri/src/storage/` | Nova função para consultar OpenVGDB (`serial → nome`) |
| `src-tauri/src/commands.rs` | Novo comando `list_synced_games` retornando `Vec<SyncedGame>` por emulador |
| `src-tauri/src/` | Asset do OpenVGDB empacotado via `tauri::include_asset!` ou path relativo |

Nova struct `SyncedGame` (espelhada em `src/types/ipc.ts`):

```rust
pub struct SyncedGame {
    pub serial: String,
    pub name: Option<String>,   // None se o serial não estiver na base
    pub emulator: String,
    pub categories: Vec<SyncCategory>,
    pub last_synced_at_ms: i64,
    pub size_bytes: i64,
}
```

### Frontend (React/TS)

- Novo comando `listSyncedGames` em `src/lib/ipc.ts`.
- Interface `SyncedGame` em `src/types/ipc.ts`.
- Componente de lista de jogos por emulador na tela principal.

---

## Limitações conhecidas

- Jogos sem serial na base do OpenVGDB exibem o código bruto (`ULUS12345`).
- PCSX2 memory cards não têm granularidade por jogo — apareceriam como `Mcd001.ps2`.
- Savestates do PPSSPP (`PPSSPP_STATE/`) seguem o padrão `<SERIAL>_<SLOT>.ppst` —
  identificáveis, mas cobertos indiretamente pelos saves da mesma pasta `SAVEDATA/`.

---

## Relação com outros documentos

- Arquitetura da boundary IPC: [`docs/referencia-ipc.md`](../referencia-ipc.md)
- Perfis de emulador: [`src-tauri/src/emulator/ppsspp.rs`](../../src-tauri/src/emulator/ppsspp.rs), [`pcsx2.rs`](../../src-tauri/src/emulator/pcsx2.rs)
- Manifest SQLite: [`src-tauri/src/storage/manifest.rs`](../../src-tauri/src/storage/manifest.rs)
