use crossbeam::channel;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};
use futures::{task, Stream, StreamExt};

fn main() {
    let mut mini_tokio = MiniTokio::new();
    // mini_tokio.spawn(async {
    //     let when = Instant::now() + Duration::from_millis(10);
    //     let future = Delay { when, waker: None };
    //
    //     let out = future.await;
    //     assert_eq!(out, "done");
    // });
    mini_tokio.spawn(async {
        let mut interval = Interval {
            rem: 3,
            delay: Delay {
                when: Instant::now(),
                waker: None,
            },
        };

        while let Some(_) =interval.next().await {
            println!("Interval!");
        }
    });
    mini_tokio.run();
}

struct Delay {
    when: Instant,
    waker: Option<Arc<Mutex<Waker>>>,
}

// 为我们的 Delay 类型实现 Future 特征
impl Future for Delay {
    type Output = &'static str;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<&'static str> {
        if let Some(waker) = &self.waker {
            let mut waker = waker.lock().unwrap();

            if !waker.will_wake(cx.waker()) {
                *waker = cx.waker().clone();
            }
        } else {
            let when = self.when;
            let waker = Arc::new(Mutex::new(cx.waker().clone()));
            self.waker = Some(waker.clone());

            thread::spawn(move || {
                let now = Instant::now();
                if now < when {
                    thread::sleep(when - now);
                }
                waker.lock().unwrap().wake_by_ref();
            });
        }

        if Instant::now() >= self.when {
            // 时间到了，Future 可以结束
            Poll::Ready("done")
        } else {
            Poll::Pending
        }
    }
}

struct Interval {
    rem: usize,
    delay: Delay,
}

impl Stream for Interval {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.rem == 0 {
            return Poll::Ready(None);
        };
        match Pin::new(&mut self.delay).poll(cx) {
            Poll::Ready(_) => {
                let when = self.delay.when + Duration::from_millis(1000);
                self.delay = Delay { when, waker: None };
                self.rem -= 1;
                Poll::Ready(Some(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.rem, None)
    }
}

struct MiniTokio {
    scheduled: channel::Receiver<Arc<Task>>,
    sender: channel::Sender<Arc<Task>>,
}

impl Task {
    fn schedule(self: &Arc<Self>) {
        self.executor.send(self.clone()).unwrap();
    }

    fn poll(self: Arc<Self>) {
        // 基于 Task 实例创建一个 waker, 它使用了之前的 `ArcWake`
        let waker = task::waker(self.clone());
        let mut cx = Context::from_waker(&waker);

        // 没有其他线程在竞争锁时，我们将获取到目标 future
        let mut future = self.future.try_lock().unwrap();

        // 对 future 进行 poll
        let _ = future.as_mut().poll(&mut cx);
    }

    // 使用给定的 future 来生成新的任务
    //
    // 新的任务会被推到 `sender` 中，接着该消息通道的接收端就可以获取该任务，然后执行
    fn spawn<F>(future: F, sender: &channel::Sender<Arc<Task>>)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task = Arc::new(Task {
            future: Mutex::new(Box::pin(future)),
            executor: sender.clone(),
        });

        let _ = sender.send(task);
    }
}
struct Task {
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,
    executor: channel::Sender<Arc<Task>>,
}

use futures::task::ArcWake;

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.schedule();
    }
}

impl MiniTokio {
    fn new() -> MiniTokio {
        let (sender, scheduled) = channel::unbounded();
        MiniTokio { sender, scheduled }
    }

    /// 生成一个 Future并放入 mini-tokio 实例的任务队列中
    fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Task::spawn(future, &self.sender)
    }

    fn run(self) {
        drop(self.sender);
        while let Ok(task) = self.scheduled.recv() {
            task.poll();
        }
    }
}
