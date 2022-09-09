use std::ops::ControlFlow;

use apollo_compiler::ApolloCompiler;
use apollo_router::graphql::Error;
use apollo_router::layers::ServiceBuilderExt;
use apollo_router::plugin::{Plugin, PluginInit};
use apollo_router::register_plugin;
use apollo_router::services::supergraph;
use http::StatusCode;
use schemars::JsonSchema;
use serde::Deserialize;
use tower::util::BoxService;
use tower::{BoxError, ServiceBuilder, ServiceExt};

use crate::compiler_ext::CompilerAdditions;
use crate::operation_depth::OperationDefinitionExt;

#[derive(Debug)]
struct BasicDepthLimit {
    configuration: Conf,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
struct Conf {
    limit: usize,
}

#[async_trait::async_trait]
impl Plugin for BasicDepthLimit {
    type Config = Conf;

    async fn new(init: PluginInit<Self::Config>) -> Result<Self, BoxError> {
        Ok(BasicDepthLimit {
            configuration: init.config,
        })
    }

    fn supergraph_service(
        &self,
        service: BoxService<supergraph::Request, supergraph::Response, BoxError>,
    ) -> BoxService<supergraph::Request, supergraph::Response, BoxError> {
        let limit = self.configuration.limit;
        ServiceBuilder::new()
            .checkpoint(move |req: supergraph::Request| {
                if let Some(operation) = req.supergraph_request.body().query.clone() {
                    let ctx = ApolloCompiler::new(&operation);
                    let operation_name = req.supergraph_request.body().operation_name.as_deref();

                    if let Some(operation) = ctx.operation_by_name(operation_name) {
                        let depth = operation.max_depth(&ctx);

                        tracing::debug!(?operation_name, %depth, "operation_depth");

                        if depth > limit {
                            let error = Error::builder()
                                .message("operation depth exceeded limit")
                                .build();

                            let res = supergraph::Response::builder()
                                .error(error)
                                .status_code(StatusCode::BAD_REQUEST)
                                .context(req.context)
                                .build()?;

                            return Ok(ControlFlow::Break(res));
                        }
                    } else {
                        tracing::warn!("could not find operation in document");
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
register_plugin!("apollosolutions", "basic_depth_limit", BasicDepthLimit);

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
                "apollosolutions.basic_depth_limit": {
                    "limit" : 10,
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
    async fn basic_test() -> Result<(), BoxError> {
        let test_harness = TestHarness::builder()
            .configuration_json(serde_json::json!({
            "plugins": {
                "apollosolutions.basic_depth_limit": {
                    "limit" : 10,
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

        let next = streamed_response.next_response().await;
        assert!(next.is_none());
        Ok(())
    }
}
