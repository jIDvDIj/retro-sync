# BUG-001 — Perda de save no primeiro sync em dispositivo com jogo já instalado

**Status:** ✅ resolvido (v1.1 · Passo 6 — ver [13-primeiro-sync-backup.md](../13-primeiro-sync-backup.md))  
**Severidade:** crítica (perda irreversível de dados)  
**Componente:** `src-tauri/src/sync/conflict.rs`, `src-tauri/src/sync/engine.rs`

> **Resolução:** `decide` retorna `DownloadWithBackup` quando `last_synced = None` e o arquivo
> existe nos dois lados — o Drive vence e o local é copiado para `<app_data>/backups/...` antes de
> ser sobrescrito. Adotou-se a combinação **A + B** das opções abaixo (Drive-first no primeiro
> sync + backup local). O sufixo de conflito (regra de first-sync-as-conflict) **não** foi
> adotado: optou-se por Drive-vence-com-backup, mais simples e sem poluir a pasta de saves.

---

## Descrição

Quando o usuário instala o RetroSync em um segundo dispositivo que já possui saves locais
do emulador, o sync inicial pode sobrescrever silenciosamente o save mais valioso (que está
no Drive) pelo save local mais recente em termos de mtime, sem qualquer aviso.

## Reprodução

1. **Dispositivo A** — usuário joga Monster Hunter por 100 horas, o save é sincronizado ao
   Drive via RetroSync. O manifest do Dispositivo A registra `(local_mtime=T₁, drive_mtime=T₁)`.

2. **Dispositivo B** — usuário instala o PPSSPP e o mesmo jogo. O emulador cria um save novo
   ao iniciar o jogo pela primeira vez. O usuário joga 20 minutos. O arquivo de save tem
   `mtime = hoje` (mais recente que o save de 100 h no Drive).

3. Usuário instala o RetroSync no Dispositivo B e faz login. O manifest local está **vazio**
   (primeira execução).

4. O sync de startup (`Bidirectional`) é disparado automaticamente.

5. Para o save de Monster Hunter, `decide()` recebe:
   - `local_mtime_ms` = hoje (mtime do save de 20 min)
   - `drive_mtime_ms` = dias atrás (quando o Dispositivo A sincronizou)
   - `last_synced` = `None` (manifest vazio)

6. `decide()` compara mtime bruto: `local > drive` → **`Upload`**.

7. O save de 20 minutos **sobrescreve** o save de 100 horas no Drive. Operação irreversível.

## Causa raiz

A função `decide()` em `conflict.rs` usa `last_synced` para distinguir "arquivo não mudou
desde o último sync" de "arquivo mudou". Quando `last_synced = None`, ela cai no caminho
de comparação direta de mtime:

```rust
// conflict.rs:36-44
if eq_within_tolerance(local, drive) {
    SyncAction::NoOp
} else if local > drive {
    SyncAction::Upload   // ← salvo local mais recente em mtime vence
} else {
    SyncAction::Download
}
```

Em uma primeira execução, o mtime do arquivo local reflete quando o emulador o criou/modificou
naquele dispositivo — não quando o progresso de jogo foi feito. Um save recém-criado tem mtime
recente mesmo que represente pouco progresso, enquanto o save no Drive pode ter mtime mais antigo
e representar centenas de horas de jogo.

## Impacto

- Perda permanente de progresso de jogo no Drive.
- Ocorre silenciosamente: nenhum evento de erro ou aviso é emitido ao frontend.
- Afeta qualquer emulador suportado (PPSSPP, PCSX2), não só Monster Hunter.
- Reproduzível sempre que o usuário tem o jogo instalado antes do RetroSync.

## Soluções consideradas

### Opção A — Drive-first no primeiro sync (recomendada como base)

Quando `last_synced = None` e o arquivo existe nos dois lados, forçar `Download`
independente de mtime. Drive é a fonte de verdade na primeira execução.

**Prós:** mudança mínima em `conflict.rs`, sem UI nova.  
**Contras:** ignora saves locais legítimos que possam ser mais novos (caso raro, mas possível).

### Opção B — Backup local antes de sobrescrever (primeiro sync)

Quando `last_synced = None` e o arquivo existe nos dois lados, antes de executar o
`Download`, copiar o arquivo local para `<nome>.retrosync.bak`. O usuário pode recuperar
manualmente. O backup é restrito ao primeiro sync — syncs subsequentes não fazem backup,
pois o usuário já tinha ciência do estado sincronizado anteriormente.

**Prós:** não-destrutivo dos dois lados; encaixa no princípio já adotado para o Drive;
escopo limitado evita acúmulo de backups desnecessários.  
**Contras:** não evita a sobrescrita, só permite recuperação.

### Opção C — Conflito explícito via UI

Quando `last_synced = None` e ambos existem, não fazer nada e emitir um evento de conflito
para o frontend exibir as duas versões (tamanho, data) e deixar o usuário escolher.

**Prós:** decisão correta sempre.  
**Contras:** exige trabalho no frontend; interrompe o fluxo automático.

### Relação com a solução do BUG-002

O [BUG-002](./bug-002-conflito-edicao-simultanea.md) adota as Opções 1 + 3: sufixo de
conflito para preservar as duas versões e backup antes de sobrescrever. Essa combinação
**cobre parcialmente** este bug — o backup (Opção 3) evita a perda irreversível, mas o
sufixo de conflito (Opção 1 do BUG-002) **não é acionado automaticamente** aqui.

A Opção 1 do BUG-002 só dispara quando `decide()` detecta conflito real, ou seja, quando
ambos os lados mudaram desde `last_synced`. No primeiro sync, `last_synced = None` e o
código cai diretamente na comparação de mtime — o caminho de conflito nunca é alcançado.

Para que o mesmo mecanismo cubra os dois bugs, é necessária uma regra adicional em
`decide()`: **quando `last_synced = None` e o arquivo existe nos dois lados, tratar sempre
como conflito**, independente de mtime. Com isso, o sufixo de conflito seria aplicado e
nenhuma versão seria perdida.

### Solução recomendada

Combinar **A + B + regra de first-sync-as-conflict**:

1. Quando `last_synced = None` e o arquivo existe nos dois lados → tratar como conflito
   (mesmo mecanismo do BUG-002, Opção 1): renomear o local com sufixo e baixar o do Drive.
2. Antes do download destrutivo no primeiro sync (`last_synced = None`) → backup local
   (Opção B acima). Syncs subsequentes não fazem backup.

Assim os dois bugs são resolvidos pelo mesmo mecanismo, sem UI nova e de forma
não-destrutiva dos dois lados.

## Arquivos a modificar

| Arquivo | Mudança |
|---|---|
| `src-tauri/src/sync/conflict.rs` | Novo parâmetro `has_manifest: bool`; quando `false` e ambos existem → `Download` |
| `src-tauri/src/sync/diff.rs` | Passar `has_manifest` para `decide()`; fazer backup local antes de download destrutivo |
| `src-tauri/src/sync/engine.rs` | Garantir que backup seja registrado no log de progresso |

## Referências

- Implementação de `decide()`: [`src-tauri/src/sync/conflict.rs:19`](../../src-tauri/src/sync/conflict.rs)
- Construção do plano: [`src-tauri/src/sync/diff.rs:104`](../../src-tauri/src/sync/diff.rs)
- Princípio não-destrutivo: [`docs/decisoes-tecnicas.md`](../decisoes-tecnicas.md)
