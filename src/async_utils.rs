use tokio::sync::mpsc::{channel, Receiver, Sender};

use std::cell::RefCell;

enum Side {
    SideA,
    SideB,
}

pub struct TwinBarrier {
    tx_go: RefCell<Sender<bool>>,
    rx_go: RefCell<Receiver<bool>>,
    side: Side,
}

impl TwinBarrier {
    async fn tx_go(&self) -> Result<(), &'static str> {
        match self.tx_go.borrow_mut().send(true).await {
            Ok(_) => Ok(()),
            _ => Err("Twin barrier locking down."),
        }
    }

    async fn rx_go(&self) -> Result<(), &'static str> {
        match self.rx_go.borrow_mut().recv().await {
            Some(_) => Ok(()),
            None => Err("Twin barrier locking down."),
        }
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
    pub async fn wait(&self) -> Result<(), &'static str> {
        match self.side {
            Side::SideA => {
                self.tx_go().await?;
                self.rx_go().await?;
            }
            Side::SideB => {
                self.rx_go().await?;
                self.tx_go().await?;
            }
        }

        Ok(())
    }
}

pub fn twin_barrier() -> (TwinBarrier, TwinBarrier) {
    let (tx_a, rx_b) = channel(1);
    let (tx_b, rx_a) = channel(1);

    let side_a = TwinBarrier {
        tx_go: RefCell::new(tx_a),
        rx_go: RefCell::new(rx_a),
        side: Side::SideA,
    };

    let side_b = TwinBarrier {
        tx_go: RefCell::new(tx_b),
        rx_go: RefCell::new(rx_b),
        side: Side::SideB,
    };

    (side_a, side_b)
}
