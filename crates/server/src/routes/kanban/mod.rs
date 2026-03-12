use axum::Router;
use serde::Serialize;

use crate::DeploymentImpl;

mod issue_assignees;
mod issue_comment_reactions;
mod issue_comments;
mod issue_followers;
mod issue_relationships;
mod issue_tags;
mod issues;
mod notifications;
mod organizations;
mod project_statuses;
mod projects;
mod pull_requests;
mod tags;
mod users;
mod workspaces;

#[derive(Serialize)]
pub struct MutationResponse<T: Serialize> {
    data: T,
    txid: i64,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .merge(organizations::router())
        .merge(users::router())
        .merge(projects::router())
        .merge(project_statuses::router())
        .merge(issues::router())
        .merge(tags::router())
        .merge(issue_assignees::router())
        .merge(issue_followers::router())
        .merge(issue_tags::router())
        .merge(issue_relationships::router())
        .merge(issue_comments::router())
        .merge(issue_comment_reactions::router())
        .merge(pull_requests::router())
        .merge(workspaces::router())
        .merge(notifications::router())
}
