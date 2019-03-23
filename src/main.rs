use futures;
use futures::sync::oneshot;
use futures::Async;
use hyper::rt::Future;
use hyper::service::service_fn_ok;
use hyper::{Body, Response, Server};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use stopwatch;

const TEXT: &str = "Hello, World!";
const SLEEP_TIMER_SECS: u64 = 30;


struct ShutdownSignaler {
    state: Arc<Mutex<bool>>,
}

impl ShutdownSignaler {
    fn send(&self, _: ()) { // to match API of oneshot::Sender::send()
        match self.state.lock() {
            Ok(mut is_signaled) => {
                if *is_signaled == false {
                    *is_signaled = true;
                }
            }
            Err(_) => unreachable!(),
        }
    }
}

struct ShutdownReceiver {
    state: Arc<Mutex<bool>>,
}

impl Future for ShutdownReceiver {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        self.state
            .try_lock()
            .map(|is_signaled| {
                if *is_signaled == true {
                    return Async::Ready(());
                }

                return Async::NotReady;
            })
            .map_err(|_| ())
    }
}

fn custom_channel() -> (ShutdownSignaler, ShutdownReceiver) {
    let channel_state = Arc::new(Mutex::new(false));

    let tx = ShutdownSignaler {
        state: Arc::clone(&channel_state),
    };
    let rx = ShutdownReceiver {
        state: channel_state,
    };

    (tx, rx)
}


fn main() {
    let addr = ([127, 0, 0, 1], 9000).into();

    let new_svc = || service_fn_ok(|_req| Response::new(Body::from(TEXT)));

//    let (tx, rx) = oneshot::channel();

    let (tx, rx) = custom_channel(); // <- creating custom "channel"

    let server = Server::bind(&addr)
        .serve(new_svc)
        .with_graceful_shutdown(rx)
        .map_err(|e| eprintln!("server error: {}", e));

    let stopwatch = stopwatch::Stopwatch::start_new();

    // We emulating external signal source with designated thread here
    thread::spawn(move || {
        thread::sleep(Duration::new(SLEEP_TIMER_SECS, 0));

        tx.send(());

        println!("Shut down sent at {} ms", &stopwatch.elapsed_ms());
    });

    println!("Starting server at {} ms", &stopwatch.elapsed_ms());

    hyper::rt::run(server);

    println!("Server got shut down at {} ms", &stopwatch.elapsed_ms());
}