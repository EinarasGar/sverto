use dal::models::user_models::UserOnboardingModel;

#[derive(Clone, Debug)]
pub struct UserOnboardingDto {
    pub default_asset_id: Option<i32>,
    pub default_ticker: Option<String>,
    pub onboarding_version: i32,
    pub ai_features_enabled: bool,
}

impl From<UserOnboardingModel> for UserOnboardingDto {
    fn from(model: UserOnboardingModel) -> Self {
        Self {
            default_asset_id: model.default_asset,
            default_ticker: model.default_ticker,
            onboarding_version: model.onboarding_version,
            ai_features_enabled: model.ai_features_enabled,
        }
    }
}
