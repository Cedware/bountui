#[derive(Clone)]
pub enum Routes {
    Scopes { parent: Option<String> },
    Targets { scope: String },
    Sessions { scope_id: String, target_id: String },
}
