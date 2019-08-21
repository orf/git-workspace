use std::fs;

#[derive(Deserialize, Serialize, Debug)]
pub struct LockfileMetadata {
    version: i32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LockFileEntry {
    pub path: String,
    pub clone_url: String,
    pub branch: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LockFile {
    metadata: LockfileMetadata,
    entries: Vec<LockFileEntry>,
}

pub fn write_lockfile(entries: Vec<LockFileEntry>) {
    let lockfile = LockFile {
        metadata: LockfileMetadata { version: 1 },
        entries,
    };
    let toml_string = toml::to_string(&lockfile).unwrap();
    fs::write("workspace/lockfile.toml", toml_string).unwrap();
}
