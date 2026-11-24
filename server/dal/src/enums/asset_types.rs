use strum::EnumCount;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, EnumCount)]
pub enum DatabaseAssetTypes {
    Currency,
    Stock,
    Bond,
    MutualFund,
    Etf,
    Etc,
    Crypto,
    RealEstate,
    Vehicle,
}

impl DatabaseAssetTypes {
    pub fn len() -> usize {
        DatabaseAssetTypes::COUNT
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Currencies" => Some(Self::Currency),
            "Stocks" => Some(Self::Stock),
            "Bonds" => Some(Self::Bond),
            "Mutual Funds" => Some(Self::MutualFund),
            "ETFs" => Some(Self::Etf),
            "ETCs" => Some(Self::Etc),
            "Cryptocurrencies" => Some(Self::Crypto),
            "Real Estate" => Some(Self::RealEstate),
            "Vehicles" => Some(Self::Vehicle),
            _ => None,
        }
    }
}
