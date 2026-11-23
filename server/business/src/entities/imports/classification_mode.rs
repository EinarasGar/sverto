use dal::models::import_models::ImportSessionSummaryModel;

use crate::dtos::imports::import_classification_dto::ImportClassificationModeDto;

pub fn classification_mode(
    summary: &ImportSessionSummaryModel,
    ai_features_enabled: bool,
) -> ImportClassificationModeDto {
    let files = summary.pending_files
        + summary.processing_files
        + summary.failed_files
        + summary.parsed_files;
    match (ai_features_enabled, files) {
        (true, _) => ImportClassificationModeDto::Ai,
        (false, 0) => ImportClassificationModeDto::WithoutAi,
        (false, _) => ImportClassificationModeDto::RefuseWithoutAi,
    }
}
