use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Course {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// Exam date `YYYY-MM-DD` (used by sprint mode and review scheduling).
    pub exam_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCourseInput {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCourseInput {
    pub id: i64,
    pub name: Option<String>,
    pub description: Option<String>,
    /// `Some("")` clears the exam date.
    #[serde(default)]
    pub exam_date: Option<String>,
}
