use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ListResponse<T> {
    pub items: Option<Vec<T>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ItemResponse<T> {
    pub item: T,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiError {
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorResponse {
    pub status_code: u16,
    pub api_error: ApiError,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AuthenticateResponse {
    pub attributes: AuthenticateAttributes,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AuthenticateAttributes {
    pub user_id: String,
    pub token: String
}
