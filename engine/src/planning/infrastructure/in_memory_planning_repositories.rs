//! Implementations of `PlanningResultRepository` and `GeneratedTemplateRepository`.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: GAP-A-16 — planning repository impls
//!
//! In-memory, Mutex-backed: planning results keyed by execution_id (with
//! hash/template secondary indexes), and a generated-template cache keyed
//! by intent hash.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::planning::domain::error::PlanningError;
use crate::planning::domain::result::{PlanningHash, PlanningResult};
use crate::planning::infrastructure::repository::{
    GeneratedTemplateRepository, PlanningResultRepository,
};
use crate::template_generation::domain::GeneratedTemplate;

fn repository_error(detail: impl Into<String>) -> PlanningError {
    PlanningError::RepositoryError {
        detail: detail.into(),
    }
}

/// In-memory planning result repository with hash/template indexes.
pub struct InMemoryPlanningResultRepository {
    by_id: Mutex<HashMap<Uuid, PlanningResult>>,
    by_hash: Mutex<HashMap<PlanningHash, Vec<Uuid>>>,
    by_template: Mutex<HashMap<String, Vec<Uuid>>>,
}

impl InMemoryPlanningResultRepository {
    /// Create an empty repository.
    pub fn new() -> Self {
        Self {
            by_id: Mutex::new(HashMap::new()),
            by_hash: Mutex::new(HashMap::new()),
            by_template: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryPlanningResultRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlanningResultRepository for InMemoryPlanningResultRepository {
    async fn save_result(&self, result: &PlanningResult) -> Result<(), PlanningError> {
        let id = result.execution_id;
        let template_id = result.template_id.clone();
        let hash = result.planning_hash.clone();

        let mut by_id = self
            .by_id
            .lock()
            .map_err(|_| repository_error("by_id poisoned"))?;
        let mut by_hash = self
            .by_hash
            .lock()
            .map_err(|_| repository_error("by_hash poisoned"))?;
        let mut by_template = self
            .by_template
            .lock()
            .map_err(|_| repository_error("by_template poisoned"))?;

        by_id.insert(id, result.clone());
        by_hash.entry(hash).or_default().push(id);
        by_template.entry(template_id).or_default().push(id);
        Ok(())
    }

    async fn load_result(
        &self,
        execution_id: Uuid,
    ) -> Result<Option<PlanningResult>, PlanningError> {
        Ok(self
            .by_id
            .lock()
            .map_err(|_| repository_error("by_id poisoned"))?
            .get(&execution_id)
            .cloned())
    }

    async fn find_by_hash(
        &self,
        hash: &PlanningHash,
    ) -> Result<Vec<PlanningResult>, PlanningError> {
        let by_id = self
            .by_id
            .lock()
            .map_err(|_| repository_error("by_id poisoned"))?;
        let by_hash = self
            .by_hash
            .lock()
            .map_err(|_| repository_error("by_hash poisoned"))?;
        Ok(by_hash
            .get(hash)
            .map(|ids| ids.iter().filter_map(|id| by_id.get(id).cloned()).collect())
            .unwrap_or_default())
    }

    async fn list_by_template(
        &self,
        template_id: &str,
        limit: u32,
    ) -> Result<Vec<PlanningResult>, PlanningError> {
        let by_id = self
            .by_id
            .lock()
            .map_err(|_| repository_error("by_id poisoned"))?;
        let by_template = self
            .by_template
            .lock()
            .map_err(|_| repository_error("by_template poisoned"))?;
        let mut results: Vec<PlanningResult> = by_template
            .get(template_id)
            .map(|ids| ids.iter().filter_map(|id| by_id.get(id).cloned()).collect())
            .unwrap_or_default();
        results.sort_by_key(|r| std::cmp::Reverse(r.planned_at));
        results.truncate(limit as usize);
        Ok(results)
    }

    async fn delete_result(&self, execution_id: Uuid) -> Result<bool, PlanningError> {
        let mut by_id = self
            .by_id
            .lock()
            .map_err(|_| repository_error("by_id poisoned"))?;
        Ok(by_id.remove(&execution_id).is_some())
    }

    async fn count(&self) -> Result<u64, PlanningError> {
        Ok(self
            .by_id
            .lock()
            .map_err(|_| repository_error("by_id poisoned"))?
            .len() as u64)
    }

    async fn exists(&self, execution_id: Uuid) -> Result<bool, PlanningError> {
        Ok(self
            .by_id
            .lock()
            .map_err(|_| repository_error("by_id poisoned"))?
            .contains_key(&execution_id))
    }
}

/// In-memory generated-template cache keyed by intent hash.
pub struct InMemoryGeneratedTemplateRepository {
    by_hash: Mutex<HashMap<String, GeneratedTemplate>>,
}

impl InMemoryGeneratedTemplateRepository {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            by_hash: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryGeneratedTemplateRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GeneratedTemplateRepository for InMemoryGeneratedTemplateRepository {
    async fn save(
        &self,
        intent_hash: &str,
        generated: &GeneratedTemplate,
    ) -> Result<(), PlanningError> {
        self.by_hash
            .lock()
            .map_err(|_| repository_error("template cache poisoned"))?
            .insert(intent_hash.to_string(), generated.clone());
        Ok(())
    }

    async fn load_by_intent_hash(
        &self,
        intent_hash: &str,
    ) -> Result<Option<GeneratedTemplate>, PlanningError> {
        Ok(self
            .by_hash
            .lock()
            .map_err(|_| repository_error("template cache poisoned"))?
            .get(intent_hash)
            .cloned())
    }

    async fn load_by_template_id(
        &self,
        template_id: &str,
    ) -> Result<Option<GeneratedTemplate>, PlanningError> {
        Ok(self
            .by_hash
            .lock()
            .map_err(|_| repository_error("template cache poisoned"))?
            .values()
            .find(|t| t.suggested_id == template_id)
            .cloned())
    }

    async fn delete(&self, intent_hash: &str) -> Result<bool, PlanningError> {
        Ok(self
            .by_hash
            .lock()
            .map_err(|_| repository_error("template cache poisoned"))?
            .remove(intent_hash)
            .is_some())
    }

    async fn clear_cache(&self) -> Result<(), PlanningError> {
        self.by_hash
            .lock()
            .map_err(|_| repository_error("template cache poisoned"))?
            .clear();
        Ok(())
    }

    async fn cache_size(&self) -> Result<u64, PlanningError> {
        Ok(self
            .by_hash
            .lock()
            .map_err(|_| repository_error("template cache poisoned"))?
            .len() as u64)
    }

    async fn exists(&self, intent_hash: &str) -> Result<bool, PlanningError> {
        Ok(self
            .by_hash
            .lock()
            .map_err(|_| repository_error("template cache poisoned"))?
            .contains_key(intent_hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_result() -> PlanningResult {
        PlanningResult::new(
            Uuid::new_v4(),
            "tpl-1".to_string(),
            0.95,
            HashMap::new(),
            PlanningHash::new("a".repeat(64)),
            false,
            1,
            10,
            None,
        )
    }

    #[tokio::test]
    async fn test_planning_result_round_trip() {
        let repo = InMemoryPlanningResultRepository::new();
        let result = sample_result();
        let id = result.execution_id;
        repo.save_result(&result).await.unwrap();
        assert!(repo.exists(id).await.unwrap());
        assert_eq!(repo.count().await.unwrap(), 1);
        let loaded = repo.load_result(id).await.unwrap().unwrap();
        assert_eq!(loaded.template_id, "tpl-1");
        assert_eq!(repo.list_by_template("tpl-1", 5).await.unwrap().len(), 1);
        assert!(repo.delete_result(id).await.unwrap());
        assert!(!repo.exists(id).await.unwrap());
    }

    #[tokio::test]
    async fn test_template_cache_round_trip() {
        let repo = InMemoryGeneratedTemplateRepository::new();
        let generated = GeneratedTemplate {
            toml_content: "version = \"1.0.0\"".to_string(),
            suggested_id: "gen-1".to_string(),
            suggested_name: "Gen One".to_string(),
            description: "test".to_string(),
            llm_calls_used: 1,
            llm_tokens_used: 10,
        };
        repo.save("abc123", &generated).await.unwrap();
        assert!(repo.exists("abc123").await.unwrap());
        assert_eq!(repo.cache_size().await.unwrap(), 1);
        let loaded = repo.load_by_intent_hash("abc123").await.unwrap().unwrap();
        assert_eq!(loaded.suggested_id, "gen-1");
        assert!(repo.load_by_template_id("gen-1").await.unwrap().is_some());
        assert!(repo.delete("abc123").await.unwrap());
    }
}
