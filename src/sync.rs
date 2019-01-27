pub mod mpsc {
    use crate::task::wait_result;
    use std::sync::mpsc;

    pub trait ReceiverExt<T> {
        fn hydro_recv(&self) -> Result<T, mpsc::RecvError>;
        fn hydro_iter(&self) -> HydroIter<T>;
    }

    impl<T> ReceiverExt<T> for mpsc::Receiver<T> {
        fn hydro_recv(&self) -> Result<T, mpsc::RecvError> {
            wait_result(
                || self.try_recv(),
                |e| match e {
                    mpsc::TryRecvError::Empty => None,
                    mpsc::TryRecvError::Disconnected =>
                        Some(mpsc::RecvError),
                }
            )
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
