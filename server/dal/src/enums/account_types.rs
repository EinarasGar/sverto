use strum::EnumCount;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, EnumCount)]
pub enum DatabaseAccountTypes {
    Credit,
    Mortgage,
    Loan,
    Investment,
    PersonalPension,
    WorkplacePension,
    CryptoWallet,
}

impl DatabaseAccountTypes {
    pub fn len() -> usize {
        DatabaseAccountTypes::COUNT
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Credit" => Some(Self::Credit),
            "Mortgage" => Some(Self::Mortgage),
            "Loan" => Some(Self::Loan),
            "Investment" => Some(Self::Investment),
            "Personal Pension" => Some(Self::PersonalPension),
            "Workplace Pension" => Some(Self::WorkplacePension),
            "Crypto Wallet" => Some(Self::CryptoWallet),
            _ => None,
        }
    }
}
