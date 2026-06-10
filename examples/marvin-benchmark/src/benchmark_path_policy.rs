use std::path::{Path, PathBuf};

use mine_sdk::MineError;

#[cfg_attr(not(test), allow(dead_code))]
pub const BENCHMARK_PATH_POLICY: &str = "Benchmark bins resolve default datasets, references, and outputs from the repository root. Absolute CLI paths are kept as-is; relative CLI paths are rebased onto the repository root so the documented workspace commands stay reproducible without cwd-sensitive path failures.";

#[derive(Debug, Clone)]
pub struct BenchmarkPathPolicy {
    repo_root: PathBuf,
}

impl BenchmarkPathPolicy {
    pub fn discover() -> Result<Self, MineError> {
        Ok(Self {
            repo_root: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .canonicalize()?,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn from_repo_root(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    #[must_use]
    pub fn benchmarks_root(&self) -> PathBuf {
        self.repo_root.join("datasets").join("benchmarks")
    }

    #[must_use]
    pub fn resolve_cli_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.repo_root.join(path)
        }
    }

    #[must_use]
    pub fn dataset_dir(&self, dataset_id: &str) -> PathBuf {
        self.benchmarks_root().join(dataset_id)
    }

    #[must_use]
    pub fn references_dir(&self, dataset_dir: &Path) -> PathBuf {
        dataset_dir.join("references")
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn outputs_dir(&self) -> PathBuf {
        self.benchmarks_root().join("outputs")
    }
}

#[cfg(test)]
mod tests {
    use super::{BENCHMARK_PATH_POLICY, BenchmarkPathPolicy};
    use std::path::PathBuf;

    #[test]
    fn resolve_cli_path_rebases_relative_paths_on_repo_root() {
        let policy = BenchmarkPathPolicy::from_repo_root(PathBuf::from(r"C:\repo"));

        assert_eq!(
            policy.resolve_cli_path(&PathBuf::from(r"datasets\benchmarks\marvin")),
            PathBuf::from(r"C:\repo")
                .join("datasets")
                .join("benchmarks")
                .join("marvin")
        );
    }

    #[test]
    fn resolve_cli_path_preserves_absolute_paths() {
        let policy = BenchmarkPathPolicy::from_repo_root(PathBuf::from(r"C:\repo"));
        let absolute_path = PathBuf::from(r"C:\external\benchmark.json");

        assert_eq!(policy.resolve_cli_path(&absolute_path), absolute_path);
    }

    #[test]
    fn benchmark_policy_mentions_repo_root_rebasing() {
        assert!(BENCHMARK_PATH_POLICY.contains("repository root"));
        assert!(BENCHMARK_PATH_POLICY.contains("relative CLI paths are rebased"));
    }
}
