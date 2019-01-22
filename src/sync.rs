pub mod mpsc {
    use crate::task::next;
    use std::sync::mpsc;

    pub trait ReceiverExt<T> {
        fn hydro_recv(&self) -> Result<T, mpsc::RecvError>;
    }

    impl<T> ReceiverExt<T> for mpsc::Receiver<T> {
        fn hydro_recv(&self) -> Result<T, mpsc::RecvError> {
            loop {
                match self.try_recv() {
                    Ok(x) => return Ok(x),
                    Err(mpsc::TryRecvError::Empty) =>
                        next(),
                    Err(mpsc::TryRecvError::Disconnected) =>
                        return Err(mpsc::RecvError),
                }
            }
        }
    }
}
