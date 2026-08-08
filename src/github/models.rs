pub struct PRRequest {
    pub id: u32,
    pub title: String,
    pub is_draft: bool,
    pub review_status: ReviewStatus,
    pub ci_status: CIStatus,
}

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
