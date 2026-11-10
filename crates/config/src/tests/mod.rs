use super::*;
use std::fs;
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
    environment: BTreeMap<String, String>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let ca = dir.path().join("ca.crt");
        let key = dir.path().join("master.key");
        fs::write(&ca, "ca").unwrap();
        fs::write(&key, "k1 current AAAA\n").unwrap();
        let mut fixture = Self {
            dir,
            environment: BTreeMap::new(),
        };
        fixture.set("TANDEM_PUBLIC_URL", "https://tandem.example.test");
        fixture.set(
            "TANDEM_DATABASE_URL",
            "postgres://app:p4ss@db:5432/tandem?sslmode=verify-full",
        );
        fixture.set("TANDEM_DATABASE_CA_FILE", ca.to_str().unwrap());
        fixture.set("TANDEM_MASTER_KEY_FILE", key.to_str().unwrap());
        fixture
    }

    fn set(&mut self, name: &str, value: &str) -> &mut Self {
        self.environment.insert(name.to_owned(), value.to_owned());
        self
    }

    fn remove(&mut self, name: &str) -> &mut Self {
        self.environment.remove(name);
        self
    }

    fn write(&self, name: &str, text: &str) -> String {
        let path = self.dir.path().join(name);
        fs::write(&path, text).unwrap();
        path.to_str().unwrap().to_owned()
    }

    fn load(&self) -> Result<Config, ConfigError> {
        Config::load(&self.environment)
    }

    fn problems(&self) -> Vec<(String, String)> {
        self.load()
            .unwrap_err()
            .problems
            .into_iter()
            .map(|problem| (problem.setting, problem.message))
            .collect()
    }
}

mod file_and_secrets;
mod rules;
