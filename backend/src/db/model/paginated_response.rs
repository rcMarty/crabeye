#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total_count: usize,
    pub page: usize,
    pub per_page: usize,
}

impl<T> PaginatedResponse<T> {
    pub fn new(total_count: usize, page: usize, per_page: usize, items: Vec<T>) -> Self {
        Self {
            items,
            total_count,
            page,
            per_page,
        }
    }
}
