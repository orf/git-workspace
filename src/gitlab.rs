use graphql_client::{GraphQLQuery, Response};
use reqwest;

mod lockfile;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/graphql/schema.json",
    query_path = "src/graphql/query.graphql",
    response_derives = "Debug"
)]
struct AllProjectsQuery;

pub fn get_projects() {
    let q = AllProjectsQuery::build_query(all_projects_query::Variables {});
    let client = reqwest::Client::new();
    let mut res = client
        .post("https://gitlab.com/api/graphql")
        .json(&q)
        .send()
        .unwrap();
    let response_body: Response<all_projects_query::ResponseData> = res.json().unwrap();
    println!("{:#?}", response_body);
}
