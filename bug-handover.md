 # Bug: transaction embedding jobs can run before the transaction is committed

**Severity:** medium. No crash and no data corruption, but some transactions silently become invisible to AI chat's meaning-based search.
**Status:** not fixed. A fix was prototyped on `ibkr-flex-connector` and taken out so it can be done separately.

## Summary

When transactions are written inside a DB transaction (`MyraDb::start_transaction` … `commit_transaction`), their description embedding jobs are put on the job queue **immediately**, not after the commit. A worker can pick up a job before the commit. When it does, the embedding is computed and then thrown away without any error.

## Why it happens

1. `TransactionMetadataService` queues an embedding job whenever it writes a description: `transaction_metadata_service.rs` lines ~152, ~165, ~267, via `AiEmbeddingService::enqueue_embed_transaction`.
2. `JobQueueHandle::push` (`dal/src/job_queue.rs`) inserts the job through apalis with **its own connection pool**, outside the request's DB transaction. The job is visible to workers straight away.
3. The transaction rows only become visible at `commit_transaction`.
4. If the worker picks up the job first:
   - the worker calls the embedding model (AI quota is spent);
   - `store_transaction_embedding` runs `UPDATE transaction_descriptions SET embedding = … WHERE transaction_id = $1` (`ai_queries::update_transaction_description_embedding`), which matches **0 rows**;
   - Postgres treats a 0-row `UPDATE` as success, so the job "succeeds": no error, no log line, no retry.
5. Embedding jobs use `RetryPolicy::fire_and_forget()` (`worker/src/jobs/embeddings.rs`), so nothing ever fixes it later.
6. If the DB transaction **rolls back**, the jobs stay queued for rows that never existed.

## Impact

- AI chat's `query_transactions` tool searches descriptions by **meaning** using these embeddings (`ai/src/tools/query_transactions.rs`). A transaction without an embedding can still be found by keyword, but not by meaning. For example, "eating out" won't find a McDonald's row.
- There's no visible symptom: the ledger looks fine and no error is logged.
- AI quota is spent on embeddings that are discarded.

## Where it happens

Every code path that writes descriptions inside a DB transaction:

| Path | Transaction | Notes |
|---|---|---|
| Connector sync projection (`ConnectorSyncService::reconcile_and_commit`) | `connector_sync_service.rs` ~645–834 | Highest risk: hundreds of rows per sync. IBKR projects ~450 rows plus opening-buy reconciliation in one transaction, which widens the window. |
| Creating a transaction group with new transactions (`TransactionGroupService`) | `transaction_group_service.rs` ~79–122 | The group's own embedding is already queued **after** the commit (line ~125), but its child transactions' description embeddings are queued inside it. |
| Any other caller that adds or updates descriptions between `start_transaction` and `commit_transaction` | — | Same mechanism. |

Writes outside a DB transaction are not affected: rows are committed immediately.

## Why it was rarely noticed

It's a race. Small projections (Trading212, bank syncs) commit in well under a second, and the worker polls the queue on an interval, so the commit usually wins. When it loses, nothing is logged.

## How to confirm

1. Sync a connector with many described rows (or add a short `sleep` before `commit_transaction` in `reconcile_and_commit`).
2. Afterwards: `SELECT count(*) FROM transaction_descriptions WHERE embedding IS NULL;`. Rows from that sync stay `NULL`, while their embedding jobs show as completed.

## Suggested fix

Queue jobs only after the DB transaction commits, and drop them on rollback.

**Recommended (generic, fixes every path):**
- `MyraDb` keeps a list of "after-commit actions":
  - `run_after_commit(action)` adds to the list while a transaction is open, otherwise runs the action immediately;
  - `commit_transaction` runs the list after a successful commit (failures are logged, not returned, because the data is already committed);
  - `rollback_transaction` clears the list;
  - `start_transaction` also clears leftovers from a transaction a failed statement rolled back automatically.
- `JobQueueHandle` gets a per-request variant bound to that request's `MyraDb` (e.g. `bound_to(db)` in `Services::create_providers`), and `push()` goes through `run_after_commit`.
- Callers (`AiEmbeddingService` etc.) don't change.
- Watch out: in business-layer unit tests `MyraDb` is replaced by `MockMyraDb` (`mockall_double`), so the binding needs a `#[cfg(test)]` fallback to the unbound queue.

**Alternative (narrow):** in each affected service, collect the transaction ids and descriptions and call `enqueue_embed_transaction` after `commit_transaction`. `TransactionGroupService` already does this for group embeddings. It's less code, but every new transactional writer has to remember to do it.

**Optional hardening:** make the embedding job fail (and so retry) when its `UPDATE` matches 0 rows, instead of silently succeeding.

## Prototype notes

The recommended fix was implemented and tested on `ibkr-flex-connector` (commit `b8349e2`, since reverted):
- 3 files: `dal/src/database_context.rs`, `dal/src/job_queue.rs`, `business/src/service_collection.rs`. About 60 lines.
- Verified on a full IBKR re-sync: embedding jobs started only after `projection committed` in the worker log.
- Trade-off: `commit_transaction` returns slightly later, because it runs one job insert per queued job (about 450 small inserts for an IBKR sync).
