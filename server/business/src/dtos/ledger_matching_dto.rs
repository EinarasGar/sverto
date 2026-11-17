use dal::models::ledger_matching_models::{
    LedgerMovementModel, LedgerTotalModel, TransactionSourceKind, TransactionSourceModel,
};
use rust_decimal::Decimal;
use time::Date;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKindDto {
    Connector,
    Statement,
    Receipt,
    Manual,
    Balance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTraitsDto {
    pub proves_movement: bool,
    pub lists_whole_period: bool,
    pub one_per_transaction: bool,
    pub may_be_rounded: bool,
    pub reference_from_content: bool,
    pub states_position: bool,
}

impl SourceKindDto {
    pub fn traits(self) -> SourceTraitsDto {
        match self {
            Self::Connector => SourceTraitsDto {
                proves_movement: true,
                lists_whole_period: false,
                one_per_transaction: false,
                may_be_rounded: false,
                reference_from_content: false,
                states_position: false,
            },
            Self::Statement => SourceTraitsDto {
                proves_movement: true,
                lists_whole_period: true,
                one_per_transaction: false,
                may_be_rounded: false,
                reference_from_content: true,
                states_position: false,
            },
            Self::Receipt => SourceTraitsDto {
                proves_movement: false,
                lists_whole_period: false,
                one_per_transaction: true,
                may_be_rounded: false,
                reference_from_content: false,
                states_position: false,
            },
            Self::Manual => SourceTraitsDto {
                proves_movement: false,
                lists_whole_period: false,
                one_per_transaction: false,
                may_be_rounded: true,
                reference_from_content: false,
                states_position: false,
            },
            Self::Balance => SourceTraitsDto {
                proves_movement: false,
                lists_whole_period: false,
                one_per_transaction: false,
                may_be_rounded: false,
                reference_from_content: false,
                states_position: true,
            },
        }
    }
}

impl From<SourceKindDto> for TransactionSourceKind {
    fn from(value: SourceKindDto) -> Self {
        match value {
            SourceKindDto::Connector => Self::Connector,
            SourceKindDto::Statement => Self::Statement,
            SourceKindDto::Receipt => Self::Receipt,
            SourceKindDto::Manual => Self::Manual,
            SourceKindDto::Balance => Self::Balance,
        }
    }
}

impl From<TransactionSourceKind> for SourceKindDto {
    fn from(value: TransactionSourceKind) -> Self {
        match value {
            TransactionSourceKind::Connector => Self::Connector,
            TransactionSourceKind::Statement => Self::Statement,
            TransactionSourceKind::Receipt => Self::Receipt,
            TransactionSourceKind::Manual => Self::Manual,
            TransactionSourceKind::Balance => Self::Balance,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IncomingMovementDto {
    pub id: Uuid,
    pub kind: SourceKindDto,
    pub stream: String,
    pub external_ref: Option<String>,
    pub account_id: Option<Uuid>,
    pub asset_id: i32,
    pub quantity: Decimal,
    pub date: Date,
    pub description: Option<String>,
    pub itemised_fee: Option<Decimal>,
    pub is_fee: bool,
}

#[derive(Debug, Clone)]
pub struct LedgerMovementDto {
    pub transaction_id: Uuid,
    pub group_id: Option<Uuid>,
    pub date: Date,
    pub account_id: Uuid,
    pub asset_id: i32,
    pub quantity: Decimal,
    pub description: Option<String>,
    pub connector_binding_ids: Vec<Uuid>,
}

impl From<LedgerMovementModel> for LedgerMovementDto {
    fn from(model: LedgerMovementModel) -> Self {
        Self {
            transaction_id: model.id,
            group_id: model.group_id,
            date: model.date_transacted.date(),
            account_id: model.account_id,
            asset_id: model.asset_id,
            quantity: model.quantity,
            description: model.description,
            connector_binding_ids: model.connector_binding_ids,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransactionSourceDto {
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub asset_id: i32,
    pub date: Date,
    pub description: Option<String>,
    pub kind: SourceKindDto,
    pub stream: String,
    pub external_ref: Option<String>,
    pub itemised_fee: Option<Decimal>,
}

impl TryFrom<TransactionSourceModel> for TransactionSourceDto {
    type Error = anyhow::Error;

    fn try_from(model: TransactionSourceModel) -> Result<Self, Self::Error> {
        Ok(Self {
            transaction_id: model.transaction_id,
            account_id: model.account_id,
            asset_id: model.asset_id,
            date: model.occurred_on,
            description: model.description,
            kind: TransactionSourceKind::try_from(model.kind.as_str())?.into(),
            stream: model.stream,
            external_ref: model.external_ref,
            itemised_fee: model.itemised_fee,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchCandidateDto {
    pub id: Uuid,
    pub in_ledger: bool,
    pub confirmed: bool,
    pub several_transactions: bool,
    pub leaves_movement_unrecorded: bool,
    pub account_id: Option<Uuid>,
    pub asset_id: i32,
    pub quantity: Decimal,
    pub date: Date,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchOutcomeDto {
    Recorded { transaction_id: Uuid },
    Same { targets: Vec<MatchCandidateDto> },
    Ambiguous { ranked: Vec<MatchCandidateDto> },
    Parts { parts: Vec<MatchCandidateDto> },
    New,
}

#[derive(Debug, Clone)]
pub struct SettledMovementDto {
    pub movement: IncomingMovementDto,
    pub transaction_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default)]
pub struct LedgerMatchBatchDto {
    pub incoming: Vec<IncomingMovementDto>,
    pub settled: Vec<SettledMovementDto>,
}

#[derive(Debug, Clone, Default)]
pub struct LedgerActivityDto {
    pub movements: Vec<LedgerMovementDto>,
    pub sources: Vec<TransactionSourceDto>,
}

#[derive(Debug, Clone)]
pub struct LedgerTotalDto {
    pub account_id: Uuid,
    pub asset_id: i32,
    pub quantity: Decimal,
}

impl From<LedgerTotalModel> for LedgerTotalDto {
    fn from(model: LedgerTotalModel) -> Self {
        Self {
            account_id: model.account_id,
            asset_id: model.asset_id,
            quantity: model.quantity,
        }
    }
}
