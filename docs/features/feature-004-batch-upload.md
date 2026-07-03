# FEATURE-004 — Batch upload para sync inicial com coleções grandes

**Status:** ✅ implementada — `DriveClient::upload_batch` (`drive/files.rs`, `multipart/mixed`
até `DRIVE_BATCH_MAX_OPS` = 100) e o pré-passo `SyncEngine::batch_new_uploads`
(`sync/engine.rs`), que agrupa uploads de arquivos novos e pequenos e cai para o caminho
per-file no que o batch não conseguir. Ativado a partir de `DRIVE_BATCH_MIN_OPS` elegíveis.  
**Componentes afetados:** `src-tauri/src/drive/files.rs`, `src-tauri/src/sync/engine.rs`, `src-tauri/src/constants.rs`

---

## Problema atual

O `DriveClient` faz **um request HTTP por arquivo** nos uploads. Para um sync típico (dezenas
de arquivos modificados por sessão de jogo) isso é insignificante. Mas no **sync inicial** —
quando o Drive está vazio e todos os arquivos locais precisam ser enviados — uma coleção com
1 000 saves gera ~1 001 requisições:

- 1 × `files.list` para inventariar o Drive
- 1 000 × `upload_new` ou `upload_existing`, um por arquivo

O limite da Google Drive API é **1 000 req/100 segundos por usuário**. No pior caso, o sync
inicial bate nesse teto imediatamente. O `send_with_retry` atual lida com 429 via backoff
exponencial, então a operação não falha — mas pode levar vários minutos para completar.

---

## Solução proposta: Batch API do Google Drive

A [Drive Batch API](https://developers.google.com/drive/api/guides/batch) permite agrupar até
**100 operações independentes** em um único request HTTP usando o content type
`multipart/mixed`. O servidor processa cada sub-request e devolve as respostas individuais no
mesmo envelope.

Com 1 000 arquivos:

| Abordagem | Requisições HTTP |
|---|---|
| Atual (1 req/arquivo) | ~1 001 |
| Batch (100 ops/req) | ~11 |

A redução é de ~99%, eliminando praticamente o risco de rate limit no sync inicial.

### Endpoint

```
POST https://www.googleapis.com/batch/drive/v3
Content-Type: multipart/mixed; boundary=BOUNDARY

--BOUNDARY
Content-Type: application/http

POST /upload/drive/v3/files?uploadType=multipart
...

--BOUNDARY--
```

---

## Limitações conhecidas

| Limitação | Impacto |
|---|---|
| Conteúdo máximo por sub-request: **5 MB** | Arquivos de save maiores que 5 MB (raro, mas possível em savestates do PCSX2) continuam usando `upload_resumable` individualmente |
| Máximo de **100 operações por batch** | Coleções > 100 arquivos precisam de múltiplos batches; ainda reduz drasticamente o número de requests |
| Apenas uploads `multipart` dentro do batch | Não é possível iniciar sessões `resumable` dentro de um batch |

A regra de fallback é simples: arquivos acima de `SIMPLE_UPLOAD_MAX_BYTES` (constante já
existente) continuam no fluxo resumable individual; os demais entram no batch.

---

## Onde o código seria tocado

### `src-tauri/src/drive/files.rs`

Adicionar método `upload_batch`:

```rust
/// Envia até 100 arquivos pequenos (≤ SIMPLE_UPLOAD_MAX_BYTES) em um único
/// request multipart/mixed. Retorna um Vec<DriveFile> na mesma ordem.
pub async fn upload_batch(
    &self,
    ops: Vec<BatchUploadOp>,
) -> AppResult<Vec<DriveFile>> { ... }
```

Onde `BatchUploadOp` encapsula os mesmos parâmetros de `upload_new` / `upload_existing`.

### `src-tauri/src/sync/` (engine ou diff)

O ponto onde o engine itera sobre `SyncItem`s pendentes de upload precisaria agrupar os
itens elegíveis (tamanho ≤ 5 MB) em lotes de 100 antes de chamar o `DriveClient`, em vez de
chamar `upload_new` / `upload_existing` individualmente.

---

## Quando implementar

Esta é uma **melhoria de performance para coleções grandes**, não uma correção de bug. O
comportamento atual é correto — apenas mais lento no sync inicial com muitos arquivos. O
backoff exponencial já evita erros.

Prioridade recomendada: implementar antes de distribuição ampla, caso o perfil típico de
usuário inclua coleções com centenas de saves (ex.: jogadores de longa data do PPSSPP com
muitos jogos).
