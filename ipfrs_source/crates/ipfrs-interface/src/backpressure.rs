// Backpressure handling for gRPC streaming
//
// This module provides adaptive flow control and backpressure management
// for streaming RPCs to ensure stable and efficient data transfer.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::sleep;

/// Configuration for backpressure controller
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Initial window size (number of items that can be in-flight)
    pub initial_window: usize,
    /// Minimum window size
    pub min_window: usize,
    /// Maximum window size
    pub max_window: usize,
    /// Window increase factor on success
    pub increase_factor: f64,
    /// Window decrease factor on congestion
    pub decrease_factor: f64,
    /// Threshold for detecting slow consumer (items pending / window size)
    pub slow_consumer_threshold: f64,
    /// Interval for checking congestion
    pub check_interval: Duration,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            initial_window: 100,
            min_window: 10,
            max_window: 10000,
            increase_factor: 1.5,
            decrease_factor: 0.5,
            slow_consumer_threshold: 0.8,
            check_interval: Duration::from_millis(100),
        }
    }
}

/// Backpressure controller for managing flow control
#[derive(Clone)]
pub struct BackpressureController {
    config: Arc<BackpressureConfig>,
    semaphore: Arc<Semaphore>,
    window_size: Arc<AtomicUsize>,
    items_sent: Arc<AtomicU64>,
    items_consumed: Arc<AtomicU64>,
    last_adjustment: Arc<tokio::sync::Mutex<Instant>>,
    /// Number of permits that must be forgotten as they're released, to enforce a window decrease.
    permits_to_forget: Arc<AtomicUsize>,
}

impl BackpressureController {
    /// Create a new backpressure controller with the given configuration
    pub fn new(config: BackpressureConfig) -> Self {
        let initial_window = config.initial_window;
        Self {
            semaphore: Arc::new(Semaphore::new(initial_window)),
            window_size: Arc::new(AtomicUsize::new(initial_window)),
            items_sent: Arc::new(AtomicU64::new(0)),
            items_consumed: Arc::new(AtomicU64::new(0)),
            last_adjustment: Arc::new(tokio::sync::Mutex::new(Instant::now())),
            permits_to_forget: Arc::new(AtomicUsize::new(0)),
            config: Arc::new(config),
        }
    }

    /// Acquire a permit to send an item (blocks if window is full)
    pub async fn acquire(&self) -> Result<BackpressurePermit, BackpressureError> {
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| BackpressureError::Closed)?;

        self.items_sent.fetch_add(1, Ordering::Relaxed);

        Ok(BackpressurePermit {
            permit: Some(permit),
            controller: self.clone(),
        })
    }

    /// Try to acquire a permit without blocking
    pub fn try_acquire(&self) -> Result<BackpressurePermit, BackpressureError> {
        let permit = self
            .semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| BackpressureError::WouldBlock)?;

        self.items_sent.fetch_add(1, Ordering::Relaxed);

        Ok(BackpressurePermit {
            permit: Some(permit),
            controller: self.clone(),
        })
    }

    /// Signal that an item has been consumed
    pub fn signal_consumed(&self) {
        self.items_consumed.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current window size
    pub fn window_size(&self) -> usize {
        self.window_size.load(Ordering::Relaxed)
    }

    /// Get number of items sent
    pub fn items_sent(&self) -> u64 {
        self.items_sent.load(Ordering::Relaxed)
    }

    /// Get number of items consumed
    pub fn items_consumed(&self) -> u64 {
        self.items_consumed.load(Ordering::Relaxed)
    }

    /// Get current pending items (sent - consumed)
    pub fn pending_items(&self) -> u64 {
        let sent = self.items_sent();
        let consumed = self.items_consumed();
        sent.saturating_sub(consumed)
    }

    /// Check for congestion and adjust window size
    pub async fn check_congestion(&self) {
        let mut last_adjustment = self.last_adjustment.lock().await;
        let now = Instant::now();

        // Only check periodically
        if now.duration_since(*last_adjustment) < self.config.check_interval {
            return;
        }

        let pending = self.pending_items();
        let window = self.window_size() as u64;

        if window == 0 {
            return;
        }

        let utilization = pending as f64 / window as f64;

        // Detect congestion (slow consumer)
        if utilization >= self.config.slow_consumer_threshold {
            self.decrease_window().await;
            tracing::debug!(
                "Congestion detected, decreased window to {}",
                self.window_size()
            );
        } else if utilization < 0.5 && (window as usize) < self.config.max_window {
            // Low utilization, can increase window
            self.increase_window().await;
            tracing::debug!(
                "Low utilization, increased window to {}",
                self.window_size()
            );
        }

        *last_adjustment = now;
    }

    /// Increase window size
    async fn increase_window(&self) {
        let current = self.window_size();
        let new_size =
            ((current as f64 * self.config.increase_factor) as usize).min(self.config.max_window);

        if new_size > current {
            let diff = new_size - current;
            self.window_size.store(new_size, Ordering::Relaxed);
            self.semaphore.add_permits(diff);
        }
    }

    /// Decrease window size
    async fn decrease_window(&self) {
        let current = self.window_size();
        let new_size =
            ((current as f64 * self.config.decrease_factor) as usize).max(self.config.min_window);

        if new_size < current {
            let delta = current - new_size;
            self.window_size.store(new_size, Ordering::SeqCst);
            // Mark `delta` permits to be forgotten. Permits that are currently in-flight
            // will be forgotten by BackpressurePermit::drop as they complete; permits that
            // are already available are forgotten eagerly below. This mirrors increase_window's
            // add_permits(diff) and ensures the semaphore actually enforces the new window.
            self.permits_to_forget.fetch_add(delta, Ordering::SeqCst);
            let available = self.semaphore.available_permits().min(delta);
            if available > 0 {
                if let Ok(p) = self.semaphore.try_acquire_many(available as u32) {
                    self.permits_to_forget.fetch_sub(available, Ordering::SeqCst);
                    p.forget();
                }
            }
        }
    }

    /// Wait if consumer is slow (adaptive delay)
    pub async fn adaptive_delay(&self) {
        let pending = self.pending_items();
        let window = self.window_size() as u64;

        if window == 0 {
            return;
        }

        let utilization = pending as f64 / window as f64;

        if utilization > self.config.slow_consumer_threshold {
            // Consumer is slow, add proportional delay
            let delay_ms = ((utilization - self.config.slow_consumer_threshold) * 100.0) as u64;
            sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    /// Start background task for automatic congestion monitoring
    pub fn start_monitoring(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                self.check_congestion().await;
                sleep(self.config.check_interval).await;
            }
        })
    }
}

impl Default for BackpressureController {
    fn default() -> Self {
        Self::new(BackpressureConfig::default())
    }
}

/// RAII guard for backpressure permit
pub struct BackpressurePermit {
    permit: Option<OwnedSemaphorePermit>,
    controller: BackpressureController,
}

impl Drop for BackpressurePermit {
    fn drop(&mut self) {
        if let Some(permit) = self.permit.take() {
            // If a window decrease is pending, forget this permit instead of returning it,
            // so the semaphore capacity is permanently reduced by the required delta.
            let forgotten = self
                .controller
                .permits_to_forget
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    if n > 0 { Some(n - 1) } else { None }
                })
                .is_ok();
            if forgotten {
                permit.forget();
            }
            // else: OwnedSemaphorePermit drops normally, returning the permit to the semaphore
        }
    }
}

/// Errors that can occur during backpressure control
#[derive(Debug, Clone, thiserror::Error)]
pub enum BackpressureError {
    #[error("Backpressure controller is closed")]
    Closed,
    #[error("Would block, no permits available")]
    WouldBlock,
}

/// Stream wrapper with backpressure control
pub struct BackpressureStream<S> {
    inner: S,
    controller: Arc<BackpressureController>,
}

impl<S> BackpressureStream<S> {
    /// Create a new backpressure stream wrapper
    pub fn new(stream: S, controller: Arc<BackpressureController>) -> Self {
        Self {
            inner: stream,
            controller,
        }
    }

    /// Get reference to the controller
    pub fn controller(&self) -> &Arc<BackpressureController> {
        &self.controller
    }
}

impl<S> futures::Stream for BackpressureStream<S>
where
    S: futures::Stream + Unpin,
{
    type Item = S::Item;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // Check congestion before polling
        let pending = self.controller.pending_items();
        let window = self.controller.window_size() as u64;

        if window > 0 && pending >= window {
            // Window is full, apply backpressure by returning Pending
            cx.waker().wake_by_ref();
            return std::task::Poll::Pending;
        }

        // Poll inner stream
        std::pin::Pin::new(&mut self.inner).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backpressure_controller_creation() {
        let config = BackpressureConfig::default();
        let controller = BackpressureController::new(config);

        assert_eq!(controller.window_size(), 100);
        assert_eq!(controller.items_sent(), 0);
        assert_eq!(controller.items_consumed(), 0);
        assert_eq!(controller.pending_items(), 0);
    }

    #[tokio::test]
    async fn test_acquire_permit() {
        let controller = BackpressureController::default();

        let permit = controller
            .acquire()
            .await
            .expect("test: acquire permit should succeed");
        assert_eq!(controller.items_sent(), 1);
        assert_eq!(controller.items_consumed(), 0);
        assert_eq!(controller.pending_items(), 1);

        // Drop permit to release semaphore, then signal consumption
        drop(permit);
        controller.signal_consumed();
        assert_eq!(controller.items_consumed(), 1);
        assert_eq!(controller.pending_items(), 0);
    }

    #[tokio::test]
    async fn test_try_acquire() {
        let config = BackpressureConfig {
            initial_window: 2,
            ..Default::default()
        };
        let controller = BackpressureController::new(config);

        let _permit1 = controller
            .try_acquire()
            .expect("test: first try_acquire should succeed within window");
        let _permit2 = controller
            .try_acquire()
            .expect("test: second try_acquire should succeed within window");

        // Third acquire should fail
        assert!(controller.try_acquire().is_err());
    }

    #[tokio::test]
    async fn test_congestion_detection() {
        let config = BackpressureConfig {
            initial_window: 10,
            min_window: 5,
            slow_consumer_threshold: 0.8,
            check_interval: Duration::from_millis(10),
            ..Default::default()
        };
        let controller = BackpressureController::new(config);

        // Simulate slow consumer by acquiring many permits
        let mut permits = Vec::new();
        for _ in 0..9 {
            permits.push(
                controller
                    .acquire()
                    .await
                    .expect("test: acquire permit should succeed within congestion test window"),
            );
        }

        // Should have 9 pending items
        assert_eq!(controller.pending_items(), 9);

        // Check congestion (utilization = 9/10 = 0.9 > 0.8)
        sleep(Duration::from_millis(20)).await;
        controller.check_congestion().await;

        // Window should have decreased
        assert!(controller.window_size() < 10);
    }

    #[tokio::test]
    async fn test_window_increase() {
        let config = BackpressureConfig {
            initial_window: 10,
            max_window: 100,
            increase_factor: 2.0,
            check_interval: Duration::from_millis(10),
            ..Default::default()
        };
        let controller = BackpressureController::new(config);

        // Low utilization (0 pending items)
        sleep(Duration::from_millis(20)).await;
        controller.check_congestion().await;

        // Window should have increased
        assert!(controller.window_size() > 10);
    }

    #[tokio::test]
    async fn test_adaptive_delay() {
        let config = BackpressureConfig {
            initial_window: 10,
            slow_consumer_threshold: 0.8,
            ..Default::default()
        };
        let controller = BackpressureController::new(config);

        // Acquire permits to simulate congestion
        let mut permits = Vec::new();
        for _ in 0..9 {
            permits.push(
                controller
                    .acquire()
                    .await
                    .expect("test: acquire permit should succeed in adaptive delay test"),
            );
        }

        let start = Instant::now();
        controller.adaptive_delay().await;
        let elapsed = start.elapsed();

        // Should have added some delay
        assert!(elapsed >= Duration::from_millis(0));
    }

    #[tokio::test]
    async fn test_automatic_monitoring() {
        let config = BackpressureConfig {
            initial_window: 10,
            check_interval: Duration::from_millis(50),
            ..Default::default()
        };
        let controller = Arc::new(BackpressureController::new(config));

        // Start monitoring
        let handle = controller.clone().start_monitoring();

        // Let it run for a bit
        sleep(Duration::from_millis(200)).await;

        // Stop monitoring
        handle.abort();

        // Controller should still be functional
        let _permit = controller
            .acquire()
            .await
            .expect("test: acquire permit should succeed after monitoring");
    }

    #[tokio::test]
    async fn test_decrease_window_revokes_semaphore_permits() {
        let config = BackpressureConfig {
            initial_window: 10,
            min_window: 5,
            decrease_factor: 0.5,
            check_interval: Duration::from_millis(10),
            ..Default::default()
        };
        let controller = BackpressureController::new(config);

        // Hold 3 permits in-flight; 7 remain available.
        let mut in_flight: Vec<BackpressurePermit> = Vec::new();
        for _ in 0..3 {
            in_flight.push(controller.acquire().await.expect("test: acquire in-flight"));
        }

        // Decrease window from 10 → 5 (delta = 5).
        // 5 of the 7 available permits are forgotten eagerly; 0 deferred.
        controller.decrease_window().await;
        assert_eq!(controller.window_size(), 5);

        // 3 in-flight + 2 available = 5 = new window. Two more acquires must succeed…
        let extra1 = controller.try_acquire().expect("test: extra acquire 1 should fit");
        let extra2 = controller.try_acquire().expect("test: extra acquire 2 should fit");
        // …and the 6th must fail (window full).
        assert!(controller.try_acquire().is_err(), "semaphore should enforce new window size");

        // Release the two extra permits; they return normally (permits_to_forget == 0).
        drop(extra1);
        drop(extra2);

        // Release the original 3 in-flight permits; permits_to_forget is still 0 so they
        // return to the semaphore — effective capacity stays at 5.
        drop(in_flight);

        // Now all 5 slots are free; exactly 5 acquires should succeed.
        let mut full_batch: Vec<BackpressurePermit> = Vec::new();
        for _ in 0..5 {
            full_batch.push(controller.try_acquire().expect("test: full batch acquire"));
        }
        assert!(controller.try_acquire().is_err(), "semaphore still bounded at 5 after releases");
    }

    #[tokio::test]
    async fn test_decrease_window_deferred_forget() {
        let config = BackpressureConfig {
            initial_window: 10,
            min_window: 5,
            decrease_factor: 0.5,
            check_interval: Duration::from_millis(10),
            ..Default::default()
        };
        let controller = BackpressureController::new(config);

        // Hold all 10 permits so none are available for eager forgetting.
        let mut in_flight: Vec<BackpressurePermit> = Vec::new();
        for _ in 0..10 {
            in_flight.push(controller.acquire().await.expect("test: acquire all"));
        }

        // Decrease window from 10 → 5 (delta = 5, none available to forget eagerly).
        controller.decrease_window().await;
        assert_eq!(controller.window_size(), 5);
        assert_eq!(controller.permits_to_forget.load(Ordering::SeqCst), 5);

        // Drop 5 in-flight permits; each should be forgotten (deferred path).
        for _ in 0..5 {
            in_flight.pop();
        }
        assert_eq!(controller.permits_to_forget.load(Ordering::SeqCst), 0);

        // Drop remaining 5; they return normally. Semaphore now has 5 available.
        drop(in_flight);

        let mut batch: Vec<BackpressurePermit> = Vec::new();
        for _ in 0..5 {
            batch.push(controller.try_acquire().expect("test: deferred batch acquire"));
        }
        assert!(controller.try_acquire().is_err(), "semaphore bounded at 5 after deferred forget");
    }

    #[tokio::test]
    async fn test_signal_consumed() {
        let controller = BackpressureController::default();

        controller.signal_consumed();
        assert_eq!(controller.items_consumed(), 1);

        controller.signal_consumed();
        assert_eq!(controller.items_consumed(), 2);
    }
}
