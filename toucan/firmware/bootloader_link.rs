use core::{
    future::Future,
    sync::atomic::{AtomicU32, Ordering},
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Instant, with_timeout};

pub(super) struct Link {
    active: AtomicU32,
    id: AtomicU32,
    requests: Signal<CriticalSectionRawMutex, Request>,
    results: Signal<CriticalSectionRawMutex, (u32, bool)>,
}

#[derive(Clone, Copy)]
pub(super) struct Request {
    session: u32,
    id: u32,
    deadline: Instant,
}

pub(super) struct Session<'a> {
    link: &'a Link,
    id: u32,
}

impl Link {
    pub(super) const fn new() -> Self {
        Self {
            active: AtomicU32::new(0),
            id: AtomicU32::new(1),
            requests: Signal::new(),
            results: Signal::new(),
        }
    }

    pub(super) fn session(&self) -> Session<'_> {
        Session {
            link: self,
            id: self.next_id(),
        }
    }

    pub(super) fn available(&self) -> Option<u32> {
        match self.active.load(Ordering::Relaxed) {
            0 => None,
            id => Some(id),
        }
    }

    fn next_id(&self) -> u32 {
        self.id
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| {
                Some(id.wrapping_add(1).max(1))
            })
            .unwrap()
    }

    pub(super) async fn request(&self, session: Option<u32>) -> bool {
        let Some(session) = session.filter(|session| self.available() == Some(*session)) else {
            return false;
        };
        let id = self.next_id();
        let timeout = Duration::from_millis(1500);
        self.requests.signal(Request {
            session,
            id,
            deadline: Instant::now() + timeout,
        });
        with_timeout(timeout, async {
            loop {
                let (completed, accepted) = self.results.wait().await;
                if completed == id {
                    return accepted;
                }
            }
        })
        .await
        .unwrap_or(false)
    }
}

impl Session<'_> {
    pub(super) fn set_ready(&self, ready: bool) {
        self.link
            .active
            .store(if ready { self.id } else { 0 }, Ordering::Relaxed);
    }

    pub(super) async fn next(&self) -> Request {
        loop {
            let request = self.link.requests.wait().await;
            if request.session == self.id && self.link.available() == Some(self.id) {
                if Instant::now() < request.deadline {
                    return request;
                }
            }
            self.finish(request, false);
        }
    }

    pub(super) fn finish(&self, request: Request, accepted: bool) {
        if accepted {
            self.set_ready(false);
        }
        self.link.results.signal((request.id, accepted));
    }

    pub(super) async fn send<E>(
        &self,
        request: Request,
        operation: impl Future<Output = Result<(), E>>,
    ) -> bool {
        let timeout = Duration::from_millis(750);
        let accepted = Instant::now() + timeout <= request.deadline
            && with_timeout(timeout, operation)
                .await
                .is_ok_and(|result| result.is_ok());
        self.finish(request, accepted);
        accepted
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        self.set_ready(false);
    }
}
