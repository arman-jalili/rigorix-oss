//! Concurrent-safety tests for the DAG engine module.
//!
//! Exercises simultaneous graph mutations under RwLock.

#[cfg(test)]
mod tests {
    use crate::dag_engine::application::dto::{
        AddNodeInput, ConstructGraphInput,
    };
    use crate::dag_engine::application::service::DagGraphService;
    use crate::dag_engine::infrastructure::DefaultGraphRepository;
    use std::sync::Arc;
    use tokio::sync::Barrier;
    use uuid::Uuid;

    fn create_service() -> Arc<dyn DagGraphService> {
        let repo = Arc::new(DefaultGraphRepository::new());
        crate::dag_engine::application::factory::create_dag_service(repo)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_simultaneous_node_additions() {
        let service = create_service();
        let dag_id = Uuid::new_v4();

        // Construct graph first
        service
            .construct_graph(ConstructGraphInput { dag_id })
            .await
            .unwrap();

        let num_tasks = 10;
        let barrier = Arc::new(Barrier::new(num_tasks));
        let mut handles = Vec::new();

        for i in 0..num_tasks {
            let service = service.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                let input = AddNodeInput {
                    dag_id,
                    node_id: Uuid::new_v4(),
                    name: format!("concurrent-node-{}", i),
                    tool: "echo".to_string(),
                    input_params: serde_json::json!({"message": format!("hello-{}", i)}),
                    retry_policy: None,
                    dependencies: vec![],
                    fallback_node_id: None,
                };
                service.add_node(input).await.unwrap();
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all 10 nodes were added
        let state = service.get_execution_state(dag_id).await.unwrap();
        assert_eq!(state.node_count, 10);
    }
}
