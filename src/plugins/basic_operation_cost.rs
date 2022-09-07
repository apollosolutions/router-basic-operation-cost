use http::StatusCode;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::Arc;

use apollo_router::graphql::Error;
use apollo_router::layers::ServiceBuilderExt;
use apollo_router::plugin::{Plugin, PluginInit};
use apollo_router::register_plugin;
use apollo_router::services::supergraph;
use schemars::JsonSchema;
use serde::Deserialize;
use tower::util::BoxService;
use tower::{BoxError, ServiceBuilder, ServiceExt};

use crate::operation_cost::{operation_cost, Cost};

#[derive(Debug)]
struct BasicOperationCost {
    configuration: Conf,
    sdl: Arc<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
struct Conf {
    cost_map: HashMap<String, usize>,
    max_cost: usize,
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

    fn supergraph_service(
        &self,
        service: BoxService<supergraph::Request, supergraph::Response, BoxError>,
    ) -> BoxService<supergraph::Request, supergraph::Response, BoxError> {
        let sdl = self.sdl.clone();
        let cost_map = self.configuration.cost_map.clone();
        let max_cost = Cost::new(self.configuration.max_cost);

        ServiceBuilder::new()
            .checkpoint(move |req: supergraph::Request| {
                if let Some(operation) = req.originating_request.body().query.clone() {
                    let operation_name = req.originating_request.body().operation_name.as_deref();
                    let result = operation_cost(&sdl, &operation, operation_name, &cost_map);

                    if let Ok(cost) = result {
                        tracing::debug!(?operation_name, %cost, "operation_cost");

                        if cost > max_cost {
                            let error = Error::builder()
                                .message("operation cost exceeded limit")
                                .build();

                            let res = supergraph::Response::builder()
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

                        let res = supergraph::Response::builder()
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
    use apollo_router::services::supergraph;
    use apollo_router::TestHarness;
    use tower::BoxError;
    use tower::ServiceExt;

    #[tokio::test]
    async fn plugin_registered() {
        let config = serde_json::json!({
            "plugins": {
                "apollosolutions.basic_operation_cost": {
                    "max_cost" : 10,
                    "cost_map" : { "Query.hello": 10 }
                }
            }
        });
        apollo_router::TestHarness::builder()
            .configuration_json(config)
            .unwrap()
            .build()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn basic_test_error_response() -> Result<(), BoxError> {
        let test_harness = TestHarness::builder()
            .configuration_json(serde_json::json!({
                "plugins": {
                  "apollosolutions.basic_operation_cost": {
                    "max_cost" : 10,
                    "cost_map" : { "Query.topProducts": 10 }
                  }
                }
            }))
            .unwrap()
            .build()
            .await
            .unwrap();

        let request = supergraph::Request::canned_builder().build().unwrap();
        let mut streamed_response = test_harness.oneshot(request).await?;

        let first_response = streamed_response
            .next_response()
            .await
            .expect("couldn't get primary response");

        assert!(first_response.data.is_none());
        assert_eq!(first_response.errors.len(), 1);
        assert_eq!(
            first_response.errors.first().expect("qed").message,
            "operation cost exceeded limit"
        );

        let next = streamed_response.next_response().await;
        assert!(next.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn basic_test() -> Result<(), BoxError> {
        let test_harness = TestHarness::builder()
            .configuration_json(serde_json::json!({
                "plugins": {
                  "apollosolutions.basic_operation_cost": {
                    "max_cost" : 20,
                    "cost_map" : { "Query.topProducts": 2 }
                  }
                }
            }))
            .unwrap()
            .build()
            .await
            .unwrap();
        let request = supergraph::Request::canned_builder().build().unwrap();
        let mut streamed_response = test_harness.oneshot(request).await?;

        let first_response = streamed_response
            .next_response()
            .await
            .expect("couldn't get primary response");

        assert!(first_response.data.is_some());
        assert!(first_response.errors.is_empty());

        let next = streamed_response.next_response().await;
        assert!(next.is_none());
        Ok(())
    }
}
