use http::StatusCode;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::Arc;

use apollo_router::graphql::Error;
use apollo_router::layers::ServiceBuilderExt;
use apollo_router::plugin::{Plugin, PluginInit};
use apollo_router::register_plugin;
use apollo_router::services::{RouterRequest, RouterResponse};
use schemars::JsonSchema;
use serde::Deserialize;
use tower::util::BoxService;
use tower::{BoxError, ServiceBuilder, ServiceExt};

use crate::operation_cost::operation_cost;

#[derive(Debug)]
struct BasicOperationCost {
    configuration: Conf,
    sdl: Arc<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
struct Conf {
    cost_map: HashMap<String, i32>,
    max_cost: i32,
}

#[async_trait::async_trait]
impl Plugin for BasicOperationCost {
    type Config = Conf;

    async fn new(init: PluginInit<Self::Config>) -> Result<Self, BoxError> {
        Ok(BasicOperationCost {
            configuration: init.config,
            sdl: init.supergraph_sdl,
        })
    }

    fn router_service(
        &self,
        service: BoxService<RouterRequest, RouterResponse, BoxError>,
    ) -> BoxService<RouterRequest, RouterResponse, BoxError> {
        let sdl = self.sdl.clone();
        let cost_map = self.configuration.cost_map.clone();
        let max_cost = self.configuration.max_cost;

        ServiceBuilder::new()
            .checkpoint(move |req: RouterRequest| {
                if let Some(operation) = req.originating_request.body().query.clone() {
                    let operation_name = req.originating_request.body().operation_name.as_ref();
                    let result = operation_cost(&sdl, &operation, operation_name, &cost_map);

                    if let Ok(cost) = result {
                        tracing::debug!("cost for operation {:?}: {}", operation_name, cost);
                        if cost > max_cost {
                            let error = Error::builder()
                                .message("operation cost exceeded limit")
                                .build();

                            let res = RouterResponse::builder()
                                .error(error)
                                .status_code(StatusCode::BAD_REQUEST)
                                .context(req.context)
                                .build()?;

                            return Ok(ControlFlow::Break(res));
                        }
                    } else {
                        let error = Error::builder()
                            .message("could not calculate operation cost")
                            .build();

                        let res = RouterResponse::builder()
                            .error(error)
                            .status_code(StatusCode::INTERNAL_SERVER_ERROR)
                            .context(req.context)
                            .build()?;

                        return Ok(ControlFlow::Break(res));
                    }
                }

                Ok(ControlFlow::Continue(req))
            })
            .service(service)
            .boxed()
    }
}

// This macro allows us to use it in our plugin registry!
// register_plugin takes a group name, and a plugin name.
register_plugin!(
    "apollosolutions",
    "basic_operation_cost",
    BasicOperationCost
);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::{BasicOperationCost, Conf};

    use apollo_router::plugin::test::{IntoSchema, PluginTestHarness};
    use apollo_router::plugin::{Plugin, PluginInit};
    use tower::BoxError;

    #[tokio::test]
    async fn plugin_registered() {
        apollo_router::plugin::plugins()
            .get("apollosolutions.basic_operation_cost")
            .expect("Plugin not found")
            .create_instance(&serde_json::json!({"cost_map" : { "Query.hello": 10 }, "max_cost": 10, "sdl": "type Query { hello: String }"}), Default::default())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn basic_test_error_response() -> Result<(), BoxError> {
        // Define a configuration to use with our plugin
        let conf = Conf {
            cost_map: HashMap::from([("Query.topProducts".to_string(), 10)]),
            max_cost: 10,
        };

        let sdl = Arc::new(String::from(include_str!(
            "../../supergraph-schema.graphql"
        )));

        // Build an instance of our plugin to use in the test harness
        let plugin = BasicOperationCost::new(PluginInit::new(conf, sdl))
            .await
            .expect("created plugin");

        // Create the test harness. You can add mocks for individual services, or use prebuilt canned services.
        let mut test_harness = PluginTestHarness::builder()
            .plugin(plugin)
            .schema(IntoSchema::Canned)
            .build()
            .await?;

        // Send a request
        let mut result = test_harness.call_canned().await?;

        let first_response = result
            .next_response()
            .await
            .expect("couldn't get primary response");

        assert!(first_response.data.is_none());
        assert_eq!(first_response.errors.len(), 1);
        assert_eq!(
            first_response.errors.first().expect("qed").message,
            "operation cost exceeded limit"
        );

        // You could keep calling result.next_response() until it yields None if you're expecting more parts.
        assert!(result.next_response().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn basic_test() -> Result<(), BoxError> {
        let conf = Conf {
            cost_map: HashMap::from([("Query.topProducts".to_string(), 2)]),
            max_cost: 12,
        };

        let sdl = Arc::new(String::from(include_str!(
            "../../supergraph-schema.graphql"
        )));

        // Build an instance of our plugin to use in the test harness
        let plugin = BasicOperationCost::new(PluginInit::new(conf, sdl))
            .await
            .expect("created plugin");

        // Create the test harness. You can add mocks for individual services, or use prebuilt canned services.
        let mut test_harness = PluginTestHarness::builder()
            .plugin(plugin)
            .schema(IntoSchema::Canned)
            .build()
            .await?;

        // Send a request
        let mut result = test_harness.call_canned().await?;

        let first_response = result
            .next_response()
            .await
            .expect("couldn't get primary response");

        assert!(first_response.data.is_some());
        assert_eq!(first_response.errors.len(), 0);

        // You could keep calling result.next_response() until it yields None if you're expecting more parts.
        assert!(result.next_response().await.is_none());
        Ok(())
    }
}
