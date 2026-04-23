//! OpenTelemetry trace exporter. Activated when `OTLP_ENDPOINT` env is set.
//! Emits spans via gRPC OTLP to Jaeger (or any OTLP receiver).

use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{Resource, trace::SdkTracerProvider};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Layer;

/// If OTLP_ENDPOINT is set, build a tracer provider and return a layer to add to the subscriber.
pub fn maybe_otel_layer<S>() -> Option<Box<dyn Layer<S> + Send + Sync + 'static>>
where
    S: tracing::Subscriber + Send + Sync + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    let endpoint = std::env::var("OTLP_ENDPOINT").ok()?;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .ok()?;

    let resource = Resource::builder()
        .with_attributes([
            KeyValue::new("service.name", env!("CARGO_PKG_NAME")),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ])
        .build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    let tracer = provider.tracer(env!("CARGO_PKG_NAME"));
    global::set_tracer_provider(provider);

    Some(Box::new(OpenTelemetryLayer::new(tracer)))
}

pub fn shutdown() {
    // SDK 0.31 auto-shuts-down on drop; this is a no-op placeholder.
}
