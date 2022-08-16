use std::ops::ControlFlow;

use apollo_compiler::ApolloCompiler;
use apollo_router::graphql::Error;
use apollo_router::layers::ServiceBuilderExt;
use apollo_router::plugin::{Plugin, PluginInit};
use apollo_router::register_plugin;
use apollo_router::services::{RouterRequest, RouterResponse};
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

    fn router_service(
        &self,
        service: BoxService<RouterRequest, RouterResponse, BoxError>,
    ) -> BoxService<RouterRequest, RouterResponse, BoxError> {
        let limit = self.configuration.limit;
        ServiceBuilder::new()
            .checkpoint(move |req: RouterRequest| {
                if let Some(operation) = req.originating_request.body().query.clone() {
                    let ctx = ApolloCompiler::new(&operation);
                    let operation_name = req.originating_request.body().operation_name.as_ref();

                    if let Some(operation) = ctx.operation_by_name(operation_name) {
                        let depth = operation.max_depth(&ctx);
                        tracing::debug!("depth for operation {:?}: {}", operation_name, depth);
                        if depth > limit {
                            let error = Error::builder()
                                .message("operation depth exceeded limit")
                                .build();

                            let res = RouterResponse::builder()
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
    use super::{BasicDepthLimit, Conf};

    use apollo_router::plugin::test::IntoSchema::Canned;
    use apollo_router::plugin::test::PluginTestHarness;
    use apollo_router::plugin::{Plugin, PluginInit};
    use tower::BoxError;

    #[tokio::test]
    async fn plugin_registered() {
        apollo_router::plugin::plugins()
            .get("apollosolutions.basic_depth_limit")
            .expect("Plugin not found")
            .create_instance(&serde_json::json!({"limit" : 10}), Default::default())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn basic_test() -> Result<(), BoxError> {
        // Define a configuration to use with our plugin
        let conf = Conf { limit: 10 };

        // Build an instance of our plugin to use in the test harness
        let plugin = BasicDepthLimit::new(PluginInit::new(conf, Default::default()))
            .await
            .expect("created plugin");

        // Create the test harness. You can add mocks for individual services, or use prebuilt canned services.
        let mut test_harness = PluginTestHarness::builder()
            .plugin(plugin)
            .schema(Canned)
            .build()
            .await?;

        // Send a request
        let mut result = test_harness.call_canned().await?;

        let first_response = result
            .next_response()
            .await
            .expect("couldn't get primary response");

        assert!(first_response.data.is_some());

        // You could keep calling result.next_response() until it yields None if you're expexting more parts.
        assert!(result.next_response().await.is_none());
        Ok(())
    }
}
