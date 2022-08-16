use apollo_router::plugin::{Plugin, PluginInit};
use apollo_router::register_plugin;
use apollo_router::services::{RouterRequest, RouterResponse};
use schemars::JsonSchema;
use serde::Deserialize;
use tower::util::BoxService;
use tower::{BoxError, ServiceBuilder, ServiceExt};

#[derive(Debug)]
struct BasicDepthLimit {
    configuration: Conf,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
struct Conf {}

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
        ServiceBuilder::new().service(service).boxed()
    }
}

// This macro allows us to use it in our plugin registry!
// register_plugin takes a group name, and a plugin name.
register_plugin!("apollosolutions", "basic_depth_limit", BasicDepthLimit);

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn plugin_registered() {
        apollo_router::plugin::plugins()
            .get("apollosolutions.basic_depth_limit")
            .expect("Plugin not found")
            .create_instance(&serde_json::json!({}), Default::default())
            .await
            .unwrap();
    }
}
