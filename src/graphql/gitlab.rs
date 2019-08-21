use crate::lockfile::LockFileEntry;
use graphql_client::{GraphQLQuery, Response};
use reqwest;
use std::error::Error;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/graphql/schema.json",
    query_path = "src/graphql/query.graphql",
    response_derives = "Debug"
)]
struct AllProjectsQuery;

pub fn get_projects() -> Result<Vec<LockFileEntry>, failure::Error> {
    let q = AllProjectsQuery::build_query(all_projects_query::Variables {
        group: "gitlab-org".to_string(),
    });
    let client = reqwest::Client::new();
    let mut res = client
        .post("https://gitlab.com/api/graphql")
        .json(&q)
        .send()
        .unwrap();
    let response_body: Response<all_projects_query::ResponseData> = res.json().unwrap();
    response_body
        .data
        .unwrap()
        .group
        .unwrap()
        .projects
        .edges
        .unwrap()
        .into_iter()
        .map(|node| node.unwrap().node.unwrap())
        .filter(|node| node.repository.unwrap().root_ref.is_some())
        .for_each(|node| {
            println!(
                "{:#?}",
                LockFileEntry {
                    path: "1".to_string(),
                    clone_url: node.ssh_url_to_repo.unwrap(),
                    branch: node.repository.unwrap().root_ref.unwrap(),
                }
            )
        });
    let vec: Vec<LockFileEntry> = Vec::new();
    Ok(vec)
    //response_body.data.unwrap().group.projects?.edges
}
