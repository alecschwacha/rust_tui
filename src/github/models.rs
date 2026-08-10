#[derive(Debug, Clone, Copy)]
pub enum ReviewStatus {
    Approved,
    ChangesRequested,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub enum CIStatus {
    Success,
    Failure,
    Pending,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct PRRequest {
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub author: String,
    pub is_draft: bool,
    pub review_status: ReviewStatus,
    pub ci_status: CIStatus,
}
