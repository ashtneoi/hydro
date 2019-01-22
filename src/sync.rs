pub mod mpsc {
    use crate::task::next;
    use std::sync::mpsc;

    pub trait ReceiverExt<T> {
        fn hydro_recv(&self) -> Result<T, mpsc::RecvError>;
        fn hydro_iter(&self) -> HydroIter<T>;
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

        fn hydro_iter(&self) -> HydroIter<T> {
            HydroIter { rx: self }
        }
    }

    pub struct HydroIter<'r, T: 'r> {
        rx: &'r mpsc::Receiver<T>,
    }

    impl<'r, T: 'r> Iterator for HydroIter<'r, T> {
        type Item = T;

        fn next(&mut self) -> Option<T> {
            self.rx.hydro_recv().ok()
        }
    }
}
