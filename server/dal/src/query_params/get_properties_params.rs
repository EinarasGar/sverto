use sqlx::types::Uuid;

pub struct GetPropertiesParams {
    pub user_id: Uuid,
    pub property_id: Option<Uuid>,
}

impl GetPropertiesParams {
    pub fn all(user_id: Uuid) -> Self {
        Self {
            user_id,
            property_id: None,
        }
    }

    pub fn by_id(user_id: Uuid, property_id: Uuid) -> Self {
        Self {
            user_id,
            property_id: Some(property_id),
        }
    }
}
