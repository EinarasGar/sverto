use std::collections::HashMap;

use shared::view_models::imports::answers::{
    AccountMappingAnswer, AccountMappingAnswerChoice, AssetMappingAnswer, AssetMappingAnswerChoice,
    ImportAnswerViewModel, NoCheckpointAnswer, NoCheckpointAnswerChoice, NotOnStatementAnswer,
    NotOnStatementAnswerChoice,
};
use shared::view_models::imports::asks::{IdentifiableImportAskViewModel, ImportAskChoice};
use shared::view_models::imports::balance_checks::ImportBalanceChecksViewModel;
use shared::view_models::imports::pending::IdentifiableImportPendingTransactionViewModel;
use shared::view_models::imports::session::{
    IdentifiableImportSessionResponseViewModel, IdentifiableImportSourceViewModel,
    ImportClassificationStatus as VmClassificationStatus, ImportDateOrder as VmDateOrder,
    ImportFileStatus as VmFileStatus, ImportSessionDetailResponseViewModel,
    ImportSessionStatus as VmSessionStatus,
};
use shared::view_models::imports::staged::{
    IdentifiableStagedTransactionViewModel, ImportAmendRefusal as VmAmendRefusal, ImportItemKind,
    ImportItemVerb as VmRowVerb, ImportItemVerbOverride, ImportItemsResponseViewModel,
    ImportNearMissCandidateViewModel,
};
use shared::view_models::transactions::base_models::metadata_lookup::MetadataLookupTables;
use time::OffsetDateTime;
use uuid::Uuid;

use super::transactions::{
    find_account, find_category, find_category_icon, find_ticker, flatten_identifiable,
    primary_display, secondary_display, type_label,
};
use crate::models::{
    ImportAccountCandidate, ImportAmendRefusal, ImportAnswer, ImportAsk, ImportAssetCandidate,
    ImportBalanceChange, ImportBalanceCheck, ImportBalanceChecks, ImportCandidate,
    ImportCandidatePart, ImportClassificationStatus, ImportCounts, ImportDateOrder,
    ImportEnteredBalance, ImportFile, ImportFileStatus, ImportItemVerb, ImportItems,
    ImportPendingRow, ImportRowKind, ImportRowVerb, ImportSessionDetail, ImportSessionStatus,
    ImportSessionSummary, ImportSource, ImportSourceKind, ImportStagedRow, ImportUnlistedMovement,
    ImportUnstageable,
};
use crate::money::format_money;

fn session_status(vm: VmSessionStatus) -> ImportSessionStatus {
    match vm {
        VmSessionStatus::Collecting => ImportSessionStatus::Collecting,
        VmSessionStatus::Parsing => ImportSessionStatus::Parsing,
        VmSessionStatus::Reconciling => ImportSessionStatus::Reconciling,
        VmSessionStatus::NeedsInput => ImportSessionStatus::NeedsInput,
        VmSessionStatus::ProposalReady => ImportSessionStatus::ProposalReady,
        VmSessionStatus::Completed => ImportSessionStatus::Completed,
    }
}

fn classification_status(vm: VmClassificationStatus) -> ImportClassificationStatus {
    match vm {
        VmClassificationStatus::NotStarted => ImportClassificationStatus::NotStarted,
        VmClassificationStatus::Waiting => ImportClassificationStatus::Waiting,
        VmClassificationStatus::Running => ImportClassificationStatus::Running,
        VmClassificationStatus::Classified => ImportClassificationStatus::Classified,
        VmClassificationStatus::Failed => ImportClassificationStatus::Failed,
    }
}

pub fn date_order_to_vm(order: ImportDateOrder) -> VmDateOrder {
    match order {
        ImportDateOrder::DayFirst => VmDateOrder::DayFirst,
        ImportDateOrder::MonthFirst => VmDateOrder::MonthFirst,
    }
}

pub fn item_verb_to_vm(verb: ImportItemVerb) -> ImportItemVerbOverride {
    match verb {
        ImportItemVerb::Auto => ImportItemVerbOverride::Auto,
        ImportItemVerb::Skip => ImportItemVerbOverride::Skip,
        ImportItemVerb::Add => ImportItemVerbOverride::Add,
        ImportItemVerb::Apart => ImportItemVerbOverride::Apart,
    }
}

pub fn answer_to_vm(answer: ImportAnswer) -> Result<ImportAnswerViewModel, String> {
    let account = |choice| {
        ImportAnswerViewModel::AccountMapping(AccountMappingAnswer {
            kind: Default::default(),
            choice,
        })
    };
    let asset = |choice| {
        ImportAnswerViewModel::AssetMapping(AssetMappingAnswer {
            kind: Default::default(),
            choice,
        })
    };
    let unlisted = |choice| {
        ImportAnswerViewModel::NotOnStatement(NotOnStatementAnswer {
            kind: Default::default(),
            choice,
        })
    };
    Ok(match answer {
        ImportAnswer::AccountMap { account_id } => account(AccountMappingAnswerChoice::Map {
            account_id: account_id.parse().map_err(|_| "invalid account id")?,
        }),
        ImportAnswer::AccountExclude => account(AccountMappingAnswerChoice::Exclude),
        ImportAnswer::AssetMap { asset_id } => asset(AssetMappingAnswerChoice::Map { asset_id }),
        ImportAnswer::AssetExclude => asset(AssetMappingAnswerChoice::Exclude),
        ImportAnswer::EnterBalance { quantity } => {
            ImportAnswerViewModel::NoCheckpoint(NoCheckpointAnswer {
                kind: Default::default(),
                choice: NoCheckpointAnswerChoice::EnterBalance { quantity },
            })
        }
        ImportAnswer::DeleteUnlisted => unlisted(NotOnStatementAnswerChoice::Delete),
        ImportAnswer::KeepUnlisted => unlisted(NotOnStatementAnswerChoice::Keep),
    })
}

fn title(
    accounts: &[Uuid],
    created_at: OffsetDateTime,
    account_names: &HashMap<String, String>,
) -> String {
    let names: Vec<&str> = accounts
        .iter()
        .filter_map(|id| account_names.get(&id.to_string()))
        .map(String::as_str)
        .collect();
    if names.is_empty() {
        let month = created_at.month().to_string();
        format!(
            "Import from {} {} {}",
            created_at.day(),
            &month[..3],
            created_at.year()
        )
    } else if names.len() <= 2 {
        names.join(" + ")
    } else {
        format!("{} + {} more", names[0], names.len() - 1)
    }
}

pub fn extract_sessions(
    body: &str,
    account_names: &HashMap<String, String>,
) -> Result<Vec<ImportSessionSummary>, String> {
    let sessions: Vec<IdentifiableImportSessionResponseViewModel> =
        serde_json::from_str(body).map_err(|e| e.to_string())?;
    Ok(sessions
        .into_iter()
        .map(|s| ImportSessionSummary {
            id: s.id.to_string(),
            title: title(&s.accounts, s.created_at, account_names),
            status: session_status(s.status),
            classification_status: classification_status(s.classification.status),
            created_at: s.created_at.unix_timestamp(),
        })
        .collect())
}

pub fn extract_created_session_id(body: &str) -> Result<String, String> {
    let session: IdentifiableImportSessionResponseViewModel =
        serde_json::from_str(body).map_err(|e| e.to_string())?;
    Ok(session.id.to_string())
}

pub fn session_detail(
    session_id: &str,
    vm: ImportSessionDetailResponseViewModel,
    account_names: &HashMap<String, String>,
    file_names: &HashMap<Uuid, String>,
    tickers: &HashMap<i32, String>,
) -> ImportSessionDetail {
    let session = vm.session;
    let can_exclude = |choices: &[ImportAskChoice]| choices.contains(&ImportAskChoice::Exclude);
    ImportSessionDetail {
        summary: ImportSessionSummary {
            id: session_id.to_string(),
            title: title(&session.accounts, session.created_at, account_names),
            status: session_status(session.status),
            classification_status: classification_status(session.classification.status),
            created_at: session.created_at.unix_timestamp(),
        },
        classification_error: session.classification.error_message,
        sources: vm
            .sources
            .into_iter()
            .map(|source| match source {
                IdentifiableImportSourceViewModel::File(f) => ImportSource {
                    id: f.id.to_string(),
                    recorded_transactions: f.recorded_transactions,
                    kind: ImportSourceKind::File {
                        file: ImportFile {
                            file_id: f.file_id.to_string(),
                            name: file_names.get(&f.file_id).cloned().unwrap_or_default(),
                            status: match f.status {
                                VmFileStatus::Pending => ImportFileStatus::Pending,
                                VmFileStatus::Processing => ImportFileStatus::Processing,
                                VmFileStatus::Parsed => ImportFileStatus::Parsed,
                                VmFileStatus::Failed => ImportFileStatus::Failed,
                            },
                            error: f.error_message,
                            paused_until: f.paused_until.map(|t| t.unix_timestamp()),
                            date_order: f.date_order.map(|order| match order {
                                VmDateOrder::DayFirst => ImportDateOrder::DayFirst,
                                VmDateOrder::MonthFirst => ImportDateOrder::MonthFirst,
                            }),
                        },
                    },
                },
                IdentifiableImportSourceViewModel::BankAccount(b) => ImportSource {
                    id: b.id.to_string(),
                    recorded_transactions: b.recorded_transactions,
                    kind: ImportSourceKind::BankAccount {
                        binding_id: b.binding_id.to_string(),
                        provider_kind: b.provider_kind,
                        display_name: b.display_name,
                        currency: b.currency,
                        account_number_last4: b.account_number_last4,
                    },
                },
            })
            .collect(),
        asks: vm
            .open_asks
            .into_iter()
            .map(|ask| match ask {
                IdentifiableImportAskViewModel::AccountMapping(a) => ImportAsk::AccountMapping {
                    can_exclude: can_exclude(&a.choices),
                    ask_key: a.ask_key,
                    source_id: a.source_id.to_string(),
                    raw_account: a.raw_account,
                    candidates: a
                        .candidates
                        .into_iter()
                        .map(|c| ImportAccountCandidate {
                            id: c.id.to_string(),
                            name: c.name,
                        })
                        .collect(),
                    answer_covers_all_sources: a.answer_covers_all_sources,
                },
                IdentifiableImportAskViewModel::AssetMapping(a) => ImportAsk::AssetMapping {
                    can_exclude: can_exclude(&a.choices),
                    ask_key: a.ask_key,
                    raw: a.raw,
                    candidates: a
                        .candidates
                        .into_iter()
                        .map(|c| ImportAssetCandidate {
                            id: c.id,
                            ticker: c.ticker,
                            name: c.name,
                        })
                        .collect(),
                },
                IdentifiableImportAskViewModel::NoCheckpoint(a) => ImportAsk::NoCheckpoint {
                    ask_key: a.ask_key,
                    account_id: a.account_id.to_string(),
                    account_name: account_names
                        .get(&a.account_id.to_string())
                        .cloned()
                        .unwrap_or_default(),
                    asset_id: a.asset_id,
                    ticker: tickers.get(&a.asset_id).cloned().unwrap_or_default(),
                    as_of_date: a.as_of_date.unix_timestamp(),
                },
            })
            .collect(),
        counts: ImportCounts {
            pending: vm.counts.pending,
        },
    }
}

fn money(quantity: &str, ticker: &str) -> Result<String, String> {
    Ok(format_money(decimal(quantity)?, ticker.to_string(), false))
}

fn decimal(quantity: &str) -> Result<f64, String> {
    quantity
        .parse()
        .map_err(|e| format!("invalid decimal '{quantity}': {e}"))
}

fn candidate(
    c: ImportNearMissCandidateViewModel,
    tables: &MetadataLookupTables,
) -> Result<ImportCandidate, String> {
    let ticker = find_ticker(tables, c.asset_id);
    Ok(ImportCandidate {
        id: c.id.to_string(),
        in_ledger: c.in_ledger,
        amend: c.amend,
        leaves_movement_unrecorded: c.leaves_movement_unrecorded,
        date: c.date.unix_timestamp(),
        description: c.description,
        account_name: c
            .account_id
            .and_then(|id| find_account(tables, &id.to_string())),
        amount_display: money(&c.quantity, &ticker)?,
        parts: c
            .parts
            .into_iter()
            .map(|p| {
                Ok(ImportCandidatePart {
                    date: p.date.unix_timestamp(),
                    description: p.description,
                    amount_display: money(&p.quantity, &ticker)?,
                })
            })
            .collect::<Result<_, String>>()?,
    })
}

fn staged_row(
    row: IdentifiableStagedTransactionViewModel,
    tables: &MetadataLookupTables,
) -> Result<ImportStagedRow, String> {
    let (date, type_key, description, category_id, entries) =
        flatten_identifiable(&row.transaction);
    let first = entries
        .first()
        .ok_or_else(|| format!("staged row {} has no entries", row.id))?;
    let account = find_account(tables, &first.0).unwrap_or_default();
    let counterparty = entries
        .get(1)
        .filter(|second| second.0 != first.0)
        .and_then(|e| find_account(tables, &e.0));
    Ok(ImportStagedRow {
        id: row.id.to_string(),
        kind: match row.kind {
            ImportItemKind::Movement => ImportRowKind::Movement,
            ImportItemKind::StartingBalance => ImportRowKind::StartingBalance,
            ImportItemKind::JoinedTransfer => ImportRowKind::JoinedTransfer,
            ImportItemKind::Adjustment => ImportRowKind::Adjustment,
        },
        verb: match row.verb {
            VmRowVerb::Add => ImportRowVerb::Add,
            VmRowVerb::Amend => ImportRowVerb::Amend,
            VmRowVerb::Delete => ImportRowVerb::Delete,
            VmRowVerb::Skip => ImportRowVerb::Skip,
        },
        verb_locked: row.verb_locked,
        ask_key: row.ask_key,
        amend_refusal: row.amend_refusal.map(|refusal| match refusal {
            VmAmendRefusal::TargetGone => ImportAmendRefusal::TargetGone,
            VmAmendRefusal::NotOneEntry => ImportAmendRefusal::NotOneEntry,
            VmAmendRefusal::Unreadable => ImportAmendRefusal::Unreadable,
        }),
        type_key: type_key.to_string(),
        type_label: type_label(type_key).to_string(),
        date,
        description: description.unwrap_or_else(|| type_label(type_key).to_string()),
        account_id: first.0.clone(),
        asset_id: first.1,
        account_name: match counterparty {
            Some(other) => format!("{account} → {other}"),
            None => account,
        },
        amount_display: primary_display(&entries, tables),
        secondary_amount_display: secondary_display(&entries, tables),
        category_name: category_id.and_then(|id| find_category(tables, id)),
        category_icon: category_id.and_then(|id| find_category_icon(tables, id)),
        near_miss: row
            .near_miss_candidates
            .into_iter()
            .map(|c| candidate(c, tables))
            .collect::<Result<_, String>>()?,
    })
}

fn balance_checks(
    vm: ImportBalanceChecksViewModel,
    tables: &MetadataLookupTables,
) -> Result<ImportBalanceChecks, String> {
    let checks = vm
        .checks
        .into_iter()
        .map(|check| {
            let ticker = find_ticker(tables, check.asset_id);
            let starting_off_by = decimal(&check.starting_off_by)?;
            let final_off_by = decimal(&check.final_off_by)?;
            Ok(ImportBalanceCheck {
                account_id: check.account_id.to_string(),
                asset_id: check.asset_id,
                account_name: find_account(tables, &check.account_id.to_string())
                    .unwrap_or_default(),
                first_date: check.first_date.unix_timestamp(),
                last_date: check.last_date.unix_timestamp(),
                balances_checked: check.balances_checked,
                starting_off_by,
                starting_off_by_display: format_money(starting_off_by, ticker.clone(), false),
                final_off_by,
                final_off_by_display: format_money(final_off_by, ticker.clone(), false),
                changes: check
                    .changes
                    .into_iter()
                    .map(|change| {
                        Ok(ImportBalanceChange {
                            after: change.after.unix_timestamp(),
                            through: change.through.unix_timestamp(),
                            change_display: money(&change.change, &ticker)?,
                            explained: change.explained,
                            awaits_answer: change.awaits_answer,
                        })
                    })
                    .collect::<Result<_, String>>()?,
                unlisted: check
                    .unlisted
                    .into_iter()
                    .map(|u| {
                        Ok(ImportUnlistedMovement {
                            transaction_id: u.transaction_id.to_string(),
                            date: u.date.unix_timestamp(),
                            amount_display: money(&u.quantity, &ticker)?,
                            description: u.description,
                            may_book_later: u.may_book_later,
                            explains_difference: u.explains_difference,
                            ask_key: u.ask_key,
                            deleting: u.deleting,
                        })
                    })
                    .collect::<Result<_, String>>()?,
                ticker,
            })
        })
        .collect::<Result<_, String>>()?;
    let entered = vm
        .entered
        .into_iter()
        .map(|e| {
            Ok(ImportEnteredBalance {
                quantity_display: money(&e.quantity, &find_ticker(tables, e.asset_id))?,
                ask_key: e.ask_key,
                account_id: e.account_id.to_string(),
                asset_id: e.asset_id,
                date: e.date.unix_timestamp(),
                as_of_date: e.as_of_date.unix_timestamp(),
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(ImportBalanceChecks {
        checks,
        unchecked: vm.unchecked,
        entered,
    })
}

pub fn extract_items(body: &str) -> Result<ImportItems, String> {
    let resp: ImportItemsResponseViewModel =
        serde_json::from_str(body).map_err(|e| e.to_string())?;
    let tables = &resp.lookup_tables;
    Ok(ImportItems {
        ready: resp.ready,
        fingerprint: resp.fingerprint.map(|f| f.to_string()),
        rows: resp
            .items
            .into_iter()
            .map(|row| staged_row(row, tables))
            .collect::<Result<_, String>>()?,
        balance_checks: resp
            .balance_checks
            .map(|c| balance_checks(c, tables))
            .transpose()?,
        unstageable: resp
            .unstageable
            .into_iter()
            .map(|u| ImportUnstageable {
                date: u.date.unix_timestamp(),
                description: u.description,
                reason: u.reason,
            })
            .collect(),
    })
}

pub fn extract_pending(body: &str) -> Result<Vec<ImportPendingRow>, String> {
    let rows: Vec<IdentifiableImportPendingTransactionViewModel> =
        serde_json::from_str(body).map_err(|e| e.to_string())?;
    rows.into_iter()
        .map(|row| {
            Ok(ImportPendingRow {
                account_ref: row.account_ref,
                date: row.date.unix_timestamp(),
                description: row.description,
                amounts_display: row
                    .legs
                    .iter()
                    .map(|leg| money(&leg.quantity, &leg.asset_ref))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(", "),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unreadable_decimals_instead_of_zeroing_them() {
        let body = r#"[{"id":"01a1f950-0000-7000-8000-000000000007","source_id":"01a1f900-0000-7000-8000-000000000001","account_ref":"Revolut EUR","date":"2026-10-02T00:00:00Z","legs":[{"asset_ref":"EUR","quantity":"twelve"}],"blocked_by":[]}]"#;
        assert!(extract_pending(body).is_err());
    }
}
