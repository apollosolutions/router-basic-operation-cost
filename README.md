# Apollo Router Basic Operation Cost / Depth Plugins

**The code in this repository is experimental and has been provided for reference purposes only. Community feedback is welcome but this project may not be supported in the same way that repositories in the official [Apollo GraphQL GitHub organization](https://github.com/apollographql) are. If you need help you can file an issue on this repository, [contact Apollo](https://www.apollographql.com/contact-sales) to talk to an expert, or create a ticket directly in Apollo Studio.**

## Known limitations

- The operation cost plugin is very naive. It doesn't take lists or abstract types into account. It's more of a weighted field counter than a true cost analyzer.
- The depth limiting plugin doesn't ignore introspection queries, so the minimum depth limit is 14 for introspection to work.

<details>
  <summary>Original scaffold README</summary>

# Apollo Router project

This generated project is set up to create a custom Apollo Router binary that may include plugins that you have written.

> Note: The Apollo Router is made available under the Elastic License v2.0 (ELv2).
> Read [our licensing page](https://www.apollographql.com/docs/resources/elastic-license-v2-faq/) for more details.

# Compile the router

To create a debug build use the following command.

```bash
cargo build
```

Your debug binary is now located in `target/debug/router`

For production, you will want to create a release build.

```bash
cargo build --release
```

Your release binary is now located in `target/release/router`

# Run the Apollo Router

1. Download the example schema

   ```bash
   curl -sSL https://supergraph.demo.starstuff.dev/ > supergraph-schema.graphql
   ```

2. Run the Apollo Router

   During development it is convenient to use `cargo run` to run the Apollo Router as it will

   ```bash
   cargo run -- --hot-reload --config router.yaml --supergraph supergraph-schema.graphql
   ```

> If you are using managed federation you can set APOLLO_KEY and APOLLO_GRAPH_REF environment variables instead of specifying the supergraph as a file.

# Create a plugin

1. From within your project directory scaffold a new plugin
   ```bash
   cargo router plugin create hello_world
   ```
2. Select the type of plugin you want to scaffold:

   ```bash
   Select a plugin template:
   > "basic"
   "auth"
   "tracing"
   ```

   The different templates are:

   - basic - a barebones plugin.
   - auth - a basic authentication plugin that could make an external call.
   - tracing - a plugin that adds a custom span and a log message.

   Choose `basic`.

3. Add the plugin to the `router.yaml`

   ```yaml
   plugins:
     starstuff.hello_world:
       message: "Starting my plugin"
   ```

4. Run the Apollo Router and see your plugin start up

   ```bash
   cargo run -- --hot-reload --config router.yaml --supergraph supergraph-schema.graphql
   ```

   In your output you should see something like:

   ```bash
   2022-05-21T09:16:33.160288Z  INFO router::plugins::hello_world: Starting my plugin
   ```

# Remove a plugin

1. From within your project run the following command. It makes a best effort to remove the plugin, but your mileage may vary.
   ```bash
   cargo router plugin remove hello_world
   ```
   </details>
