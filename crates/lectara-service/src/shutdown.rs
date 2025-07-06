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

impl Default for ShutdownState {
    fn default() -> Self {
        Self::new()
    }
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
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{Barrier, Notify};
    use tower::{ServiceBuilder, ServiceExt};

    // Test timing constants
    const FAST_DELAY: Duration = Duration::from_millis(10);
    const COORDINATION_TIMEOUT: Duration = Duration::from_secs(5);

    /// Helper for deterministic delays in tests
    async fn test_delay(duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    /// A configurable echo service for testing
    #[derive(Clone)]
    struct EchoService {
        delay: Duration,
        start_notify: Option<Arc<Notify>>,
        complete_notify: Option<Arc<Notify>>,
    }

    impl EchoService {
        fn new() -> Self {
            Self {
                delay: FAST_DELAY,
                start_notify: None,
                complete_notify: None,
            }
        }

        fn with_delay(delay: Duration) -> Self {
            Self {
                delay,
                start_notify: None,
                complete_notify: None,
            }
        }

        fn with_notifications(start_notify: Arc<Notify>, complete_notify: Arc<Notify>) -> Self {
            Self {
                delay: FAST_DELAY,
                start_notify: Some(start_notify),
                complete_notify: Some(complete_notify),
            }
        }
    }

    impl Service<Request<Empty<Bytes>>> for EchoService {
        type Response = Response<Empty<Bytes>>;
        type Error = std::convert::Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: Request<Empty<Bytes>>) -> Self::Future {
            let delay = self.delay;
            let start_notify = self.start_notify.clone();
            let complete_notify = self.complete_notify.clone();

            Box::pin(async move {
                // Notify test that request started
                if let Some(notify) = start_notify {
                    notify.notify_one();
                }

                // Simulate work
                test_delay(delay).await;

                // Notify test that request completed
                if let Some(notify) = complete_notify {
                    notify.notify_one();
                }

                Ok(Response::new(Empty::new()))
            })
        }
    }

    #[tokio::test]
    async fn test_normal_request_processing() {
        let state = ShutdownState::new();
        let service = ServiceBuilder::new()
            .layer(GracefulShutdownLayer::new(state.clone()))
            .service(EchoService::new());

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
            .service(EchoService::new());

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
        let start_barrier = Arc::new(Barrier::new(4)); // 3 requests + 1 test
        let complete_barrier = Arc::new(Barrier::new(4)); // 3 requests + 1 test

        let service = ServiceBuilder::new()
            .layer(GracefulShutdownLayer::new(state.clone()))
            .service(EchoService::with_delay(Duration::from_millis(50))); // Longer delay for coordination

        // Start multiple requests
        let mut handles = vec![];
        for _ in 0..3 {
            let req = Request::builder().body(Empty::new()).unwrap();
            let svc = service.clone();
            let start_barrier = start_barrier.clone();
            let complete_barrier = complete_barrier.clone();

            handles.push(tokio::spawn(async move {
                // Wait for all requests to start
                start_barrier.wait().await;
                let result = svc.oneshot(req).await;
                // Wait for test to check in-flight count
                complete_barrier.wait().await;
                result
            }));
        }

        // Wait for all requests to start
        start_barrier.wait().await;

        // Should have 3 in-flight requests
        assert_eq!(state.in_flight_count(), 3);

        // Signal requests to complete
        complete_barrier.wait().await;

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
        let start_notify = Arc::new(Notify::new());
        let complete_notify = Arc::new(Notify::new());

        let service = ServiceBuilder::new()
            .layer(GracefulShutdownLayer::new(state.clone()))
            .service(EchoService::with_notifications(
                start_notify.clone(),
                complete_notify.clone(),
            ));

        // Start a request
        let req1 = Request::builder().body(Empty::new()).unwrap();
        let handle1 = tokio::spawn({
            let svc = service.clone();
            async move { svc.oneshot(req1).await }
        });

        // Wait for request to start
        tokio::time::timeout(COORDINATION_TIMEOUT, start_notify.notified())
            .await
            .expect("Request should start within timeout");
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
        // Test count constants
        const PRE_SHUTDOWN_REQUESTS: usize = 5;
        const POST_SHUTDOWN_REQUESTS: usize = 5;
        const TOTAL_REQUESTS: usize = PRE_SHUTDOWN_REQUESTS + POST_SHUTDOWN_REQUESTS;

        let state = ShutdownState::new();
        let start_notifications: Vec<Arc<Notify>> = (0..PRE_SHUTDOWN_REQUESTS)
            .map(|_| Arc::new(Notify::new()))
            .collect();

        let service = ServiceBuilder::new()
            .layer(GracefulShutdownLayer::new(state.clone()))
            .service(EchoService::with_delay(Duration::from_millis(50)));

        // Start pre-shutdown requests with individual notifications
        let mut pre_shutdown_handles = vec![];
        for start_notify in &start_notifications {
            let req = Request::builder().body(Empty::new()).unwrap();
            let start_notify = start_notify.clone();
            let complete_notify = Arc::new(Notify::new());

            let echo_service = EchoService::with_notifications(start_notify, complete_notify);
            let service_with_notify = ServiceBuilder::new()
                .layer(GracefulShutdownLayer::new(state.clone()))
                .service(echo_service);

            pre_shutdown_handles.push(tokio::spawn(async move {
                service_with_notify.oneshot(req).await
            }));
        }

        // Wait for all pre-shutdown requests to start
        for start_notify in &start_notifications {
            tokio::time::timeout(COORDINATION_TIMEOUT, start_notify.notified())
                .await
                .expect("Request should start within timeout");
        }

        // Verify all requests are in-flight
        assert_eq!(state.in_flight_count(), PRE_SHUTDOWN_REQUESTS);

        // Start shutdown
        state.start_shutdown();

        // Start post-shutdown requests (these should be rejected immediately)
        let mut post_shutdown_handles = vec![];
        for _ in 0..POST_SHUTDOWN_REQUESTS {
            let req = Request::builder().body(Empty::new()).unwrap();
            let svc = service.clone();
            post_shutdown_handles.push(tokio::spawn(async move { svc.oneshot(req).await }));
        }

        // Collect results
        let mut ok_count = 0;
        let mut unavailable_count = 0;

        // Pre-shutdown requests should ALL succeed (they were in-flight before shutdown)
        for handle in pre_shutdown_handles {
            let result = handle.await.unwrap().unwrap();
            match result.status() {
                StatusCode::OK => ok_count += 1,
                StatusCode::SERVICE_UNAVAILABLE => unavailable_count += 1,
                _ => panic!("Unexpected status: {}", result.status()),
            }
        }

        // Post-shutdown requests should ALL be rejected
        for handle in post_shutdown_handles {
            let result = handle.await.unwrap().unwrap();
            match result.status() {
                StatusCode::OK => ok_count += 1,
                StatusCode::SERVICE_UNAVAILABLE => unavailable_count += 1,
                _ => panic!("Unexpected status: {}", result.status()),
            }
        }

        // Verify exact counts
        assert_eq!(
            ok_count, PRE_SHUTDOWN_REQUESTS,
            "All pre-shutdown requests should succeed"
        );
        assert_eq!(
            unavailable_count, POST_SHUTDOWN_REQUESTS,
            "All post-shutdown requests should be rejected"
        );
        assert_eq!(ok_count + unavailable_count, TOTAL_REQUESTS);

        // All done
        assert_eq!(state.in_flight_count(), 0);
    }
}
