use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll};

use http::{Request, Response, StatusCode};
use http_body::Body;
use pin_project::pin_project;
use tower::{Layer, Service};

/// Shared state for tracking shutdown status and in-flight requests
#[derive(Clone)]
pub struct ShutdownState {
    is_shutting_down: Arc<AtomicBool>,
    in_flight_count: Arc<AtomicUsize>,
}

impl ShutdownState {
    /// Create a new shutdown state
    pub fn new() -> Self {
        Self {
            is_shutting_down: Arc::new(AtomicBool::new(false)),
            in_flight_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Signal that shutdown has started
    pub fn start_shutdown(&self) {
        self.is_shutting_down.store(true, Ordering::SeqCst);
    }

    /// Check if shutdown is in progress
    pub fn is_shutting_down(&self) -> bool {
        self.is_shutting_down.load(Ordering::SeqCst)
    }

    /// Get the current number of in-flight requests
    pub fn in_flight_count(&self) -> usize {
        self.in_flight_count.load(Ordering::SeqCst)
    }
}

/// Tower layer that adds graceful shutdown capability
#[derive(Clone)]
pub struct GracefulShutdownLayer {
    state: ShutdownState,
}

impl GracefulShutdownLayer {
    pub fn new(state: ShutdownState) -> Self {
        Self { state }
    }
}

impl<S> Layer<S> for GracefulShutdownLayer {
    type Service = GracefulShutdownService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GracefulShutdownService {
            inner,
            state: self.state.clone(),
        }
    }
}

/// Tower service that handles graceful shutdown
#[derive(Clone)]
pub struct GracefulShutdownService<S> {
    inner: S,
    state: ShutdownState,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for GracefulShutdownService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    ResBody: Body + Default,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = GracefulShutdownFuture<S::Future, ResBody, S::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        // Check if we're shutting down
        if self.state.is_shutting_down() {
            // Return 503 Service Unavailable
            let response = Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .body(ResBody::default())
                .expect("building empty response should not fail");

            GracefulShutdownFuture {
                kind: FutureKind::Immediate(Some(Ok(response))),
                state: self.state.clone(),
            }
        } else {
            // Increment in-flight counter
            self.state.in_flight_count.fetch_add(1, Ordering::SeqCst);

            // Process the request
            GracefulShutdownFuture {
                kind: FutureKind::Inner(self.inner.call(req)),
                state: self.state.clone(),
            }
        }
    }
}

/// Future for graceful shutdown requests
#[pin_project]
pub struct GracefulShutdownFuture<F, B, E> {
    #[pin]
    kind: FutureKind<F, B, E>,
    state: ShutdownState,
}

#[pin_project(project = FutureKindProj)]
enum FutureKind<F, B, E> {
    Inner(#[pin] F),
    Immediate(Option<Result<Response<B>, E>>),
}

impl<F, B, E> Future for GracefulShutdownFuture<F, B, E>
where
    F: Future<Output = Result<Response<B>, E>>,
    B: Body,
{
    type Output = Result<Response<B>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.kind.project() {
            FutureKindProj::Inner(fut) => {
                let result = fut.poll(cx);

                // If the future is complete, decrement the counter
                if result.is_ready() {
                    this.state.in_flight_count.fetch_sub(1, Ordering::SeqCst);
                }

                result
            }
            FutureKindProj::Immediate(response) => {
                // SAFETY: We know this is Some because we only poll once
                Poll::Ready(response.take().unwrap())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use http_body_util::Empty;
    use std::time::Duration;
    use tower::{ServiceBuilder, ServiceExt};

    /// A simple echo service for testing
    #[derive(Clone)]
    struct EchoService;

    impl Service<Request<Empty<Bytes>>> for EchoService {
        type Response = Response<Empty<Bytes>>;
        type Error = std::convert::Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: Request<Empty<Bytes>>) -> Self::Future {
            Box::pin(async {
                // Simulate some work
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(Response::new(Empty::new()))
            })
        }
    }

    #[tokio::test]
    async fn test_normal_request_processing() {
        let state = ShutdownState::new();
        let service = ServiceBuilder::new()
            .layer(GracefulShutdownLayer::new(state.clone()))
            .service(EchoService);

        // Normal request should go through
        let req = Request::builder().body(Empty::new()).unwrap();
        let response = service.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // In-flight count should be 0 after request completes
        assert_eq!(state.in_flight_count(), 0);
    }

    #[tokio::test]
    async fn test_rejects_new_requests_during_shutdown() {
        let state = ShutdownState::new();
        let service = ServiceBuilder::new()
            .layer(GracefulShutdownLayer::new(state.clone()))
            .service(EchoService);

        // Start shutdown
        state.start_shutdown();

        // New request should be rejected with 503
        let req = Request::builder().body(Empty::new()).unwrap();
        let response = service.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        // In-flight count should still be 0
        assert_eq!(state.in_flight_count(), 0);
    }

    #[tokio::test]
    async fn test_tracks_in_flight_requests() {
        let state = ShutdownState::new();
        let service = ServiceBuilder::new()
            .layer(GracefulShutdownLayer::new(state.clone()))
            .service(EchoService);

        // Start multiple requests
        let mut handles = vec![];
        for _ in 0..3 {
            let req = Request::builder().body(Empty::new()).unwrap();
            let svc = service.clone();
            handles.push(tokio::spawn(async move { svc.oneshot(req).await }));
        }

        // Give requests time to start
        tokio::time::sleep(Duration::from_millis(5)).await;

        // Should have 3 in-flight requests
        assert_eq!(state.in_flight_count(), 3);

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Should be back to 0
        assert_eq!(state.in_flight_count(), 0);
    }

    #[tokio::test]
    async fn test_graceful_shutdown_flow() {
        let state = ShutdownState::new();
        let service = ServiceBuilder::new()
            .layer(GracefulShutdownLayer::new(state.clone()))
            .service(EchoService);

        // Start a request
        let req1 = Request::builder().body(Empty::new()).unwrap();
        let handle1 = tokio::spawn({
            let svc = service.clone();
            async move { svc.oneshot(req1).await }
        });

        // Let it start
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(state.in_flight_count(), 1);

        // Start shutdown
        state.start_shutdown();

        // Try to start another request - should be rejected
        let req2 = Request::builder().body(Empty::new()).unwrap();
        let response = service.clone().oneshot(req2).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Original request should complete successfully
        let response1 = handle1.await.unwrap().unwrap();
        assert_eq!(response1.status(), StatusCode::OK);

        // All requests done
        assert_eq!(state.in_flight_count(), 0);
    }

    #[tokio::test]
    async fn test_concurrent_shutdown_and_requests() {
        let state = ShutdownState::new();
        let service = ServiceBuilder::new()
            .layer(GracefulShutdownLayer::new(state.clone()))
            .service(EchoService);

        // Start many requests concurrently
        let mut handles = vec![];
        for i in 0..10 {
            let req = Request::builder().body(Empty::new()).unwrap();
            let svc = service.clone();
            let shutdown_state = state.clone();

            handles.push(tokio::spawn(async move {
                // Half the requests will trigger shutdown
                if i == 5 {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    shutdown_state.start_shutdown();
                }
                svc.oneshot(req).await
            }));
        }

        // Collect results
        let mut ok_count = 0;
        let mut unavailable_count = 0;

        for handle in handles {
            let result = handle.await.unwrap().unwrap();
            match result.status() {
                StatusCode::OK => ok_count += 1,
                StatusCode::SERVICE_UNAVAILABLE => unavailable_count += 1,
                _ => panic!("Unexpected status"),
            }
        }

        // Should have some successful and some rejected
        assert!(ok_count > 0, "Should have some successful requests");
        assert!(unavailable_count > 0, "Should have some rejected requests");
        assert_eq!(ok_count + unavailable_count, 10);

        // All done
        assert_eq!(state.in_flight_count(), 0);
    }
}
