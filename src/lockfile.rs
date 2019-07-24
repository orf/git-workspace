#[derive(Deserialize, Serialize, Debug)]
struct LockfileMetadata {
    version: i32,
}

#[derive(Deserialize, Serialize, Debug)]
struct LockFileEntry {
    path: String,
    clone_url: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct LockFile {
    metadata: LockfileMetadata,
    entries: Vec<LockFileEntry>,
    master_branch: String,
}
