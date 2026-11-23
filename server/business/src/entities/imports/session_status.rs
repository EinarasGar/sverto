use dal::models::import_models::{ImportFileStatus, ImportSessionLifecycle};

use crate::dtos::imports::import_session_dto::ImportSessionStatusDto;

pub struct ImportSessionStatusInputs {
    lifecycle: ImportSessionLifecycle,
    file_count: i64,
    bank_account_count: i64,
    parsing_file_count: i64,
    failed_file_count: i64,
    open_ask_count: i64,
    classified: bool,
}

impl ImportSessionStatusInputs {
    pub fn new(lifecycle: ImportSessionLifecycle, classified: bool) -> Self {
        Self {
            lifecycle,
            file_count: 0,
            bank_account_count: 0,
            parsing_file_count: 0,
            failed_file_count: 0,
            open_ask_count: 0,
            classified,
        }
    }

    pub fn add_files(&mut self, status: ImportFileStatus, count: i64) {
        self.file_count += count;
        match status {
            ImportFileStatus::Pending | ImportFileStatus::Processing => {
                self.parsing_file_count += count
            }
            ImportFileStatus::Failed => self.failed_file_count += count,
            ImportFileStatus::Parsed => {}
        }
    }

    pub fn add_bank_accounts(&mut self, count: i64) {
        self.bank_account_count += count;
    }

    pub fn add_open_asks(&mut self, count: i64) {
        self.open_ask_count += count;
    }

    pub fn status(&self) -> ImportSessionStatusDto {
        match self.lifecycle {
            ImportSessionLifecycle::Completed => return ImportSessionStatusDto::Completed,
            ImportSessionLifecycle::Active => {}
        }
        if self.file_count + self.bank_account_count == 0 {
            return ImportSessionStatusDto::Collecting;
        }
        if self.parsing_file_count > 0 {
            return ImportSessionStatusDto::Parsing;
        }
        if self.failed_file_count > 0 || self.open_ask_count > 0 {
            return ImportSessionStatusDto::NeedsInput;
        }
        if !self.classified {
            return ImportSessionStatusDto::Reconciling;
        }
        ImportSessionStatusDto::ProposalReady
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_session(classified: bool) -> ImportSessionStatusInputs {
        ImportSessionStatusInputs::new(ImportSessionLifecycle::Active, classified)
    }

    #[test]
    fn a_completed_session_is_completed_whatever_else_holds() {
        let mut inputs = ImportSessionStatusInputs::new(ImportSessionLifecycle::Completed, false);
        inputs.add_files(ImportFileStatus::Processing, 1);
        inputs.add_open_asks(2);
        assert_eq!(inputs.status(), ImportSessionStatusDto::Completed);
    }

    #[test]
    fn any_pending_or_processing_file_means_parsing() {
        for status in [ImportFileStatus::Pending, ImportFileStatus::Processing] {
            let mut inputs = active_session(true);
            inputs.add_files(ImportFileStatus::Parsed, 3);
            inputs.add_files(status, 1);
            inputs.add_open_asks(1);
            assert_eq!(inputs.status(), ImportSessionStatusDto::Parsing);
        }
    }

    #[test]
    fn open_asks_need_input_even_before_classification_finishes() {
        let mut inputs = active_session(false);
        inputs.add_files(ImportFileStatus::Parsed, 2);
        inputs.add_open_asks(1);
        assert_eq!(inputs.status(), ImportSessionStatusDto::NeedsInput);
    }
}
