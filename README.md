# Knative

[![CI](https://github.com/rusty-jules/knative-rs/actions/workflows/ci.yaml/badge.svg)](https://github.com/rusty-jules/knative-rs/actions/workflows/ci.yaml)
[![Crates.io](https://img.shields.io/crates/v/knative)](https://crates.io/crates/knative)

A Rust implementation of [Knative][knative] and [Knative Eventing][keventing] custom resource defintions and objects, leveraging [kube-rs][kubers].

This implementation is *incomplete* and should be considered pre-alpha. It contains only a small subset of the full specification.

Currently, you can use this crate to manage the status of a [custom event source][keventing-custom-source] `CustomResource` in accordance with knative's expectations.

```rust
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};
use kube::{Api, CustomResource, Resource, ResourceExt};
use kube::runtime::controller::{Action, Context};
use knative::source_types::{
    SourceSpec,
    SourceStatus,
    SourceCondition,
    SinkManager,
};
use std::sync::Arc;

#[derive(Serialize, Deserialize, CustomResource, Clone, Debug, JsonSchema)]
#[kube(group = "mysource.dev", version = "v1", kind = "MySource", status = "MySourceStatus", namespaced)]
struct MySourceSpec {
    #[serde(flatten)]
    source_spec: SourceSpec,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
struct MySourceStatus {
    #[serde(flatten)]
    source_status: SourceStatus<SourceCondition>,
}

#[derive(Clone)]
struct Data {
    client: kube::Client,
}

async fn reconcile(my_resource: Arc<MySource>, ctx: Context<Data>) -> Result<Action, kube::Error> {
    let client = ctx.get_ref().client.clone();
    let api = Api::<MySource>::namespaced(
        client.clone(),
        my_resource.namespace().as_ref().unwrap()
    );
    let mut resource = api.get_status(&my_resource.name()).await?;

    if let Some(ref mut status) = resource.status {
        status.source_status.mark_sink("http://hardcoded-sink".parse().unwrap());

        // ...set the K_SINK environment variable on the receive-adapter that this controller manages

        // ...patch the new status with the api
    }

    Ok(Action::requeue(std::time::Duration::from_secs(60 * 60)))
}
```

Additional reference usage of this crate is currently WIP!

[knative]: https://knative.dev/docs/
[keventing]: https://github.com/knative/eventing
[keventing-custom-source]: https://knative.dev/docs/eventing/custom-event-source/custom-event-source/#required-components
[kubers]: https://github.com/kube-rs/kube-rs
