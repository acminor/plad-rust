use tokio::sync::{
    mpsc::{Receiver, Sender, channel},
};

use std::cell::RefCell;

pub struct TwinBarrier {
    tx_go: RefCell<Sender<bool>>,
    rx_go: RefCell<Receiver<bool>>,
    side: bool,
}

impl TwinBarrier {
    async fn tx_go(&self) {
        self.tx_go.borrow_mut().send(true).await;
    }

    async fn rx_go(&self) {
        self.rx_go.borrow_mut().recv().await;
    }
    // NOTE this will serve as explanation of other barriers
    // -- order is really important
    //
    // 1) block waiting for main to send
    // 2) main blocks waiting for tick to send
    // 3) tick sends after unblock, freeing main
    // -- thus we have a barrier
    //
    // oxo---
    // \/     o = send, x = block, - = unblocked
    // xo----
    pub async fn wait(&self) {
        match self.side {
            true => {
                self.tx_go().await;
                self.rx_go().await;
            },
            false => {
                self.rx_go().await;
                self.tx_go().await;
            }
        }
    }
}

pub fn twin_barrier() -> (TwinBarrier, TwinBarrier) {
    let (tx_a, rx_b) = channel(1);
    let (tx_b, rx_a)= channel(1);

    let side_a = TwinBarrier {
        tx_go: RefCell::new(tx_a),
        rx_go: RefCell::new(rx_a),
        side: true,
    };

    let side_b = TwinBarrier {
        tx_go: RefCell::new(tx_b),
        rx_go: RefCell::new(rx_b),
        side: false,
    };

    (side_a, side_b)
}
