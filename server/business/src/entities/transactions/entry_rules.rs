use rust_decimal::Decimal;

use crate::dtos::entry_dto::EntryDto;
use crate::dtos::validation_error_dto::{BusinessFieldErrorDto, BusinessValidationErrorDto};

pub(crate) fn positive(entry: &EntryDto, field: &str) -> Option<BusinessFieldErrorDto> {
    (entry.quantity <= Decimal::ZERO).then(|| error(field, "amount", "Must be a positive value."))
}

pub(crate) fn negative(entry: &EntryDto, field: &str) -> Option<BusinessFieldErrorDto> {
    (entry.quantity >= Decimal::ZERO).then(|| error(field, "amount", "Must be a negative value."))
}

pub(crate) fn non_zero(entry: &EntryDto, field: &str) -> Option<BusinessFieldErrorDto> {
    entry
        .quantity
        .is_zero()
        .then(|| error(field, "amount", "Must not be zero."))
}

pub(crate) fn same_account(
    entry: &EntryDto,
    field: &str,
    other: &EntryDto,
    other_field: &str,
) -> [Option<BusinessFieldErrorDto>; 2] {
    pair_error(
        entry.account_id != other.account_id,
        field,
        other_field,
        "account_id",
        format!("{field}.account_id and {other_field}.account_id must reference the same account."),
    )
}

pub(crate) fn different_accounts(
    entry: &EntryDto,
    field: &str,
    other: &EntryDto,
    other_field: &str,
) -> [Option<BusinessFieldErrorDto>; 2] {
    pair_error(
        entry.account_id == other.account_id,
        field,
        other_field,
        "account_id",
        format!(
            "{field}.account_id and {other_field}.account_id must reference different accounts."
        ),
    )
}

pub(crate) fn same_asset(
    entry: &EntryDto,
    field: &str,
    other: &EntryDto,
    other_field: &str,
) -> [Option<BusinessFieldErrorDto>; 2] {
    pair_error(
        entry.asset_id != other.asset_id,
        field,
        other_field,
        "asset_id",
        format!("{field}.asset_id and {other_field}.asset_id must reference the same asset."),
    )
}

pub(crate) fn equal_magnitude(
    entry: &EntryDto,
    field: &str,
    other: &EntryDto,
    other_field: &str,
) -> [Option<BusinessFieldErrorDto>; 2] {
    pair_error(
        entry.quantity + other.quantity != Decimal::ZERO,
        field,
        other_field,
        "amount",
        format!("{field}.amount and {other_field}.amount must have equal magnitude."),
    )
}

pub(crate) fn check(
    errors: impl IntoIterator<Item = Option<BusinessFieldErrorDto>>,
) -> anyhow::Result<()> {
    let errors: Vec<BusinessFieldErrorDto> = errors.into_iter().flatten().collect();
    if errors.is_empty() {
        return Ok(());
    }
    Err(BusinessValidationErrorDto { errors }.into())
}

fn pair_error(
    violated: bool,
    field: &str,
    other_field: &str,
    attribute: &str,
    message: String,
) -> [Option<BusinessFieldErrorDto>; 2] {
    if !violated {
        return [None, None];
    }
    [
        Some(error(field, attribute, &message)),
        Some(error(other_field, attribute, &message)),
    ]
}

fn error(field: &str, attribute: &str, message: &str) -> BusinessFieldErrorDto {
    BusinessFieldErrorDto {
        field: format!("{field}.{attribute}"),
        message: message.to_string(),
    }
}
