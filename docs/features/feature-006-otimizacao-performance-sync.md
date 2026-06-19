# FEATURE-006 — Otimização de performance do sync (chamadas à Drive API)

**Status:** proposta (parte já implementada — ver abaixo)
**Componentes afetados:** `src-tauri/src/drive/client.rs`, `src-tauri/src/drive/folders.rs`, `src-tauri/src/drive/files.rs`, `src-tauri/src/sync/engine.rs`, `src-tauri/src/storage/`

---

## Onde o tempo de um sync é gasto

O tempo de um sync se divide em **dois tipos** de operação, com perfis de custo bem diferentes:

| Tipo | O que é | Do que depende | Como reduzir |
|---|---|---|---|
| **Metadados** | Resolver a cadeia de pastas (`ensure_*`), listar a árvore remota (`list_tree`), achar arquivos (`find_child`) | **Latência de round-trip HTTPS** (~100–300 ms por chamada), independente do tamanho do dado | Menos chamadas / chamadas mais leves |
| **Transferência** | Upload/download do conteúdo de saves e savestates | **Tamanho do arquivo × banda** | Paralelizar |

Consequência prática:

- **Sync incremental** (o caso comum — nada ou poucos arquivos mudaram desde a última sessão):
  dominado pelo **overhead de metadados**. As chamadas de listagem/resolução são, em boa parte,
  **sequenciais** (é preciso resolver o ID da pasta antes de listar/baixar), então a latência soma
  em série. Aqui, cortar chamadas tem o **maior impacto relativo** — pode transformar "alguns
  segundos" em "quase instantâneo".
- **Sync com muito dado novo** (ex.: primeiro sync de uma coleção grande): dominado pela
  **transferência**. Cortar metadados ajuda pouco em termos proporcionais; o caminho é
  **paralelizar** os uploads/downloads.

A redução de chamadas, portanto, **reduz o tempo de sync** — principalmente no caso incremental,
que é a maioria — mas **não acelera** a transferência de arquivos grandes.

---

## O que já está implementado

Boa parte das otimizações de baixo custo já existe no código atual:

| Otimização | Onde | Estado |
|---|---|---|
| **`fields` parciais** nas listagens/uploads (`files(id,name,mimeType,modifiedTime,size,appProperties)` em vez do payload completo) | `LIST_FIELDS` / `FILE_FIELDS` em `drive/mod.rs` | ✅ Em uso |
| **Cache de IDs de pasta** (evita re-resolver `RetroSync/<Emulador>/<categoria>/...` a cada acesso dentro de um sync) | `folder_cache: RwLock<HashMap<String, String>>` em `drive/client.rs`, populado por `ensure_folder_cached` em `drive/folders.rs` | ⚠️ Existe, mas **em memória** (volátil — ver abaixo) |
| **Transferências concorrentes** (uploads/downloads em paralelo) | `stream::iter(...).buffer_unordered(DRIVE_MAX_CONCURRENT_TRANSFERS)` em `sync/engine.rs`; constante atual = `3` | ✅ Em uso |

Ou seja: as recomendações "use `fields` parciais" e "paralelize transferências" **já estão feitas**.
O que sobra é mais específico.

---

## O que ainda dá para otimizar

### 1. Persistir o cache de IDs de pasta no SQLite

O `folder_cache` é um `HashMap` em memória, vivo apenas enquanto o `DriveClient` existe — ou seja,
**é zerado a cada reinício do app**. Como o gatilho `startup` dispara um sync logo ao abrir, o
**primeiro sync após cada inicialização** re-resolve toda a cadeia de pastas via `find_folder`
(uma chamada `files.list` por segmento: raiz → emulador → categoria → subpastas). Para vários
emuladores e árvores profundas, são várias chamadas sequenciais de pura latência.

**Proposta:** persistir o mapa `cache_key → folder_id` numa tabela SQLite (ex.: `drive_folders`),
carregando-o ao construir o `DriveClient` e gravando em cada `ensure_folder_cached` que resolve um
ID novo. O sync de startup passa a pular a re-resolução. Invalidação: um ID persistido que retornar
404/`notFound` numa operação é descartado e re-resolvido (cobre o caso de a pasta ter sido movida
ou apagada manualmente no Drive). Combina com a fonte de verdade já no SQLite (tabela
`sync_manifest`), mantendo o padrão "estado operacional mora no banco local".

### 2. Reduzir o custo do `list_tree`

`list_tree` (`drive/files.rs`) faz **uma chamada `files.list` por pasta** da árvore remota — é o
maior custo de metadados do sync incremental, e escala com o número de subpastas, não com o número
de arquivos mudados. Opções a avaliar:

- Uma query mais ampla por categoria em vez de varredura pasta-a-pasta, reconstruindo a hierarquia
  a partir do campo `parents` no cliente.
- Pular `list_tree` quando o `sync_manifest` local indicar que nada mudou desde o último sync
  (otimização de "early-out"), tratando o snapshot como dica e só listando quando há sinal de
  divergência.

### 3. Ajustar `DRIVE_MAX_CONCURRENT_TRANSFERS`

O teto de concorrência é `3` (`constants.rs`). É um valor conservador. Para coleções com muitos
arquivos pequenos, elevá-lo (ex.: 6–8) encurta o tempo total de transferência, desde que o
`send_with_retry` continue absorvendo eventuais `429`/`rateLimitExceeded` com backoff. Vale medir
antes de fixar — ganho depende do perfil de arquivos e da banda do usuário.

---

## Relação com outras propostas

- [FEATURE-004 — Batch upload](./feature-004-batch-upload.md) ataca o **outro lado** (reduzir o
  número de requests de **upload** no sync inicial via Batch API). FEATURE-006 foca em **metadados**
  (resolução de pastas, listagem). As duas são complementares e podem ser feitas em qualquer ordem.

---

## Quando implementar

São **melhorias de performance**, não correções — o comportamento atual é correto, só mais lento do
que poderia no sync de startup e em árvores grandes. A persistência do cache de IDs (item 1) é a de
melhor relação custo/benefício e a recomendada como primeiro passo, especialmente antes de
distribuição ampla, quando reinícios frequentes do app (uso típico de bandeja) tornam o re-resolver
de pastas um custo recorrente.
