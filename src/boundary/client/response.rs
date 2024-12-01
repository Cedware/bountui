use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ListResponse<T> {
    pub items: Option<Vec<T>>,
}

#[derive(Deserialize, Debug)]
pub struct ItemResponse<T> {
    pub item: T,
}

#[derive(Deserialize, Debug)]
pub struct ApiError {
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct ErrorResponse {
    pub status_code: u16,
    pub api_error: ApiError,
}

#[derive(Deserialize, Debug)]
pub struct AuthenticateResponse {
    pub attributes: AuthenticateAttributes,
}

#[derive(Deserialize, Debug)]
pub struct AuthenticateAttributes {
    pub user_id: String,
}
