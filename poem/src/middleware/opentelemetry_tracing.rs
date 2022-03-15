use std::sync::Arc;

use libopentelemetry::{
    global,
    trace::{FutureExt, Span, SpanKind, TraceContextExt, Tracer},
    Context,
};
use opentelemetry_http::HeaderExtractor;
use opentelemetry_semantic_conventions::{resource, trace};

use crate::{
    web::headers::HeaderMapExt, Endpoint, IntoResponse, Middleware, Request, Response, Result,
};

/// Middleware for tracing with OpenTelemetry.
#[cfg_attr(docsrs, doc(cfg(feature = "opentelemetry")))]
pub struct OpenTelemetryTracing<T> {
    tracer: Arc<T>,
}

impl<T> OpenTelemetryTracing<T> {
    /// Create `OpenTelemetryTracing` middleware with `tracer`.
    pub fn new(tracer: T) -> Self {
        Self {
            tracer: Arc::new(tracer),
        }
    }
}

impl<T, E> Middleware<E> for OpenTelemetryTracing<T>
where
    T: Tracer + Send + Sync,
    T::Span: Send + Sync + 'static,
    E: Endpoint,
{
    type Output = OpenTelemetryTracingEndpoint<T, E>;

    fn transform(&self, ep: E) -> Self::Output {
        OpenTelemetryTracingEndpoint {
            tracer: self.tracer.clone(),
            inner: ep,
        }
    }
}

/// Endpoint for `OpenTelemetryTracing` middleware.
#[cfg_attr(docsrs, doc(cfg(feature = "opentelemetry")))]
pub struct OpenTelemetryTracingEndpoint<T, E> {
    tracer: Arc<T>,
    inner: E,
}

#[async_trait::async_trait]
impl<T, E> Endpoint for OpenTelemetryTracingEndpoint<T, E>
where
    T: Tracer + Send + Sync,
    T::Span: Send + Sync + 'static,
    E: Endpoint,
{
    type Output = Response;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        let parent_cx = global::get_text_map_propagator(|propagator| {
            propagator.extract(&HeaderExtractor(req.headers()))
        });

        let mut attributes = Vec::new();
        attributes.push(resource::TELEMETRY_SDK_NAME.string(env!("CARGO_CRATE_NAME")));
        attributes.push(resource::TELEMETRY_SDK_VERSION.string(env!("CARGO_PKG_VERSION")));
        attributes.push(resource::TELEMETRY_SDK_LANGUAGE.string("rust"));
        attributes.push(trace::HTTP_METHOD.string(req.method().to_string()));
        attributes.push(trace::HTTP_TARGET.string(req.uri().path().to_string()));
        attributes.push(trace::HTTP_CLIENT_IP.string(req.remote_addr().to_string()));
        attributes.push(trace::HTTP_FLAVOR.string(format!("{:?}", req.version())));

        let mut span = self
            .tracer
            .span_builder(format!("{} {}", req.method(), req.uri()))
            .with_kind(SpanKind::Server)
            .with_attributes(attributes)
            .start_with_context(&*self.tracer, &parent_cx);

        span.add_event("request.started".to_string(), vec![]);

        async move {
            let res = self.inner.call(req).await;
            let cx = Context::current();
            let span = cx.span();

            match res {
                Ok(resp) => {
                    let resp = resp.into_response();
                    span.add_event("request.completed".to_string(), vec![]);
                    span.set_attribute(trace::HTTP_STATUS_CODE.i64(resp.status().as_u16() as i64));
                    if let Some(content_length) =
                        resp.headers().typed_get::<headers::ContentLength>()
                    {
                        span.set_attribute(
                            trace::HTTP_RESPONSE_CONTENT_LENGTH.i64(content_length.0 as i64),
                        );
                    }
                    Ok(resp)
                }
                Err(err) => {
                    span.add_event(
                        "request.error".to_string(),
                        vec![trace::EXCEPTION_MESSAGE.string(err.to_string())],
                    );
                    Err(err)
                }
            }
        }
        .with_context(Context::current_with_span(span))
        .await
    }
}
